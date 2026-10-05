#!/usr/bin/env python3
"""Run the opt-in AC10 acquisition probe in isolated Linux test processes.

Compile acquisition_resources with Cargo first; this runner neither builds nor
downloads anything. Its summary is scoped evidence, not a production RAM budget.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import statistics
import subprocess
import sys
import tempfile


MIB = 1024 * 1024
PREFIX = "PUMAS_RESOURCE_RESULT:"
CASES = (
    ("small-single", 8 * MIB, 1, False),
    ("small-pair", 8 * MIB, 2, False),
    ("large-single", 128 * MIB, 1, False),
    ("large-pair", 128 * MIB, 2, False),
    ("retained-payload-control", 128 * MIB, 2, True),
)


def measure(binary, root, case):
    name, size, workers, control = case
    command = [
        str(binary),
        "--exact",
        "resource_probe",
        "--ignored",
        "--nocapture",
        "--test-threads=1",
    ]
    # subprocess.run kills and waits for this owned child on timeout. The source
    # and all acquisition tasks are inside it, with no external service children.
    # This parent-owned directory also cleans up the disposable fixture when a
    # failed or timed-out child cannot run its own tempfile destructor.
    with tempfile.TemporaryDirectory(prefix=f"pumas-{name}-", dir=root) as temporary:
        environment = {
            **os.environ,
            "PUMAS_RESOURCE_ROOT": temporary,
            "PUMAS_RESOURCE_BYTES": str(size),
            "PUMAS_RESOURCE_WORKERS": str(workers),
            "PUMAS_RESOURCE_RETAIN_PAYLOAD": str(int(control)),
        }
        result = subprocess.run(
            command, env=environment, capture_output=True, text=True, timeout=330
        )
    if result.returncode:
        raise RuntimeError(
            f"{name}: resource probe failed ({result.returncode}):\n{result.stdout}\n{result.stderr}"
        )
    # libtest can print its "test ..." prefix on the same line with one thread.
    rows = [line.split(PREFIX, 1)[1] for line in result.stdout.splitlines() if PREFIX in line]
    if len(rows) != 1:
        raise RuntimeError(
            f"{name}: expected exactly one measurement; a zero-test run is not evidence"
        )
    row = json.loads(rows[0])
    expected = {
        "payload_bytes_per_worker": size,
        "workers": workers,
        "retain_payload_control": control,
    }
    if row["schema_version"] != 1 or any(
        row["configured"][key] != value for key, value in expected.items()
    ):
        raise RuntimeError(f"{name}: workload identity mismatch")
    if not row["checks"] or not all(value is True for value in row["checks"].values()):
        raise RuntimeError(f"{name}: incomplete correctness checks")
    measured = row["measured"]
    if (
        measured["source_payload_bytes_written"] != size * workers
        or measured["final_payload_logical_bytes"] != size * workers
    ):
        raise RuntimeError(f"{name}: payload accounting mismatch")
    peak = measured["process_after"]["VmHWM"]
    if control and peak < size * workers:
        raise RuntimeError(f"{name}: deliberately resident payload is absent from peak RSS")
    row["case"] = name
    row["derived"] = {
        "peak_rss_growth_from_baseline_bytes": max(0, peak - measured["process_before"]["VmHWM"]),
        "process_io_deltas": {
            key: measured["process_after"][key] - measured["process_before"][key]
            for key in ("rchar", "wchar", "read_bytes", "write_bytes")
        },
    }
    return row


def summarize(rows):
    summary = {}
    for name, _, _, _ in CASES:
        values = [row["measured"]["process_after"]["VmHWM"] for row in rows if row["case"] == name]
        summary[name] = {
            "peak_rss_bytes": {
                "minimum": min(values),
                "median": statistics.median(values),
                "maximum": max(values),
            }
        }
    return summary


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--binary",
        type=Path,
        required=True,
        help="compiled acquisition_resources integration-test executable",
    )
    parser.add_argument(
        "--root",
        type=Path,
        required=True,
        help="existing directory on the filesystem being measured; owned temporary children only",
    )
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--repeats", type=int, default=3, choices=range(1, 6))
    args = parser.parse_args()
    if sys.platform != "linux":
        parser.error("this evidence requires Linux proc and inode/allocation counters")
    binary = args.binary.resolve(strict=True)
    root = args.root.resolve(strict=True)
    if not binary.is_file() or not os.access(binary, os.X_OK) or not root.is_dir():
        parser.error("binary must be executable and root must be an existing directory")
    if not args.output.parent.is_dir():
        parser.error("output parent directory must exist")
    rows = []
    for repeat in range(args.repeats):
        for case in CASES:
            row = measure(binary, root, case)
            row["repeat"] = repeat + 1
            rows.append(row)
            print(
                f"{case[0]} repeat {repeat + 1}: peak RSS {row['measured']['process_after']['VmHWM']} bytes",
                flush=True,
            )
    with binary.open("rb") as executable:
        binary_hash = hashlib.file_digest(executable, "sha256").hexdigest()
    output = {
        "schema_version": 1,
        "host": {
            "system": platform.system(),
            "kernel": platform.release(),
            "machine": platform.machine(),
            "page_size_bytes": os.sysconf("SC_PAGE_SIZE"),
            "filesystem_device": root.stat().st_dev,
        },
        "binary": {"path": str(binary), "sha256": binary_hash},
        "measurement_root": str(root),
        "scope": "fresh-process Linux synthetic loopback/public acquisition test; not release/native-provider/model-import qualification",
        "production_budget": None,
        "samples": rows,
        "summary": summarize(rows),
    }
    args.output.write_text(json.dumps(output, indent=2) + "\n")


if __name__ == "__main__":
    main()
