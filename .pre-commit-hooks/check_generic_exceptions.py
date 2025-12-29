#!/usr/bin/env python3
"""
Pre-commit hook to detect generic exception handlers in backend code.

This enforces the use of specific exception types for clearer error handling.
"""

import re
import sys
from pathlib import Path

GENERIC_EXCEPTION_PATTERNS = [
    re.compile(r"^\s*except\s+Exception\b"),
    re.compile(r"^\s*except\s*\(\s*Exception\b"),
    re.compile(r"^\s*except\s*:\s*(#.*)?$"),
]


def check_file(file_path: Path) -> list[tuple[int, str]]:
    """
    Check a file for generic exception handlers.

    Args:
        file_path: Path to the Python file to check

    Returns:
        List of (line_number, line_content) tuples for violations
    """
    violations: list[tuple[int, str]] = []

    try:
        with open(file_path, "r", encoding="utf-8") as f:
            for line_num, line in enumerate(f, start=1):
                stripped = line.strip()
                if not stripped or stripped.startswith("#"):
                    continue

                if "noqa: generic-exception" in line or "noqa:generic-exception" in line:
                    continue

                for pattern in GENERIC_EXCEPTION_PATTERNS:
                    if pattern.search(line):
                        violations.append((line_num, line.rstrip()))
                        break
    except OSError as e:
        print(f"Error reading {file_path}: {e}", file=sys.stderr)
        return []

    return violations


def main() -> int:
    """
    Main entry point for the pre-commit hook.

    Returns:
        0 if no violations found, 1 otherwise
    """
    if len(sys.argv) < 2:
        print("Usage: check_generic_exceptions.py <file1> [file2] ...", file=sys.stderr)
        return 1

    files_to_check = [Path(f) for f in sys.argv[1:]]
    total_violations = 0

    for file_path in files_to_check:
        if file_path.suffix != ".py":
            continue
        if not str(file_path).startswith("backend/"):
            continue

        violations = check_file(file_path)
        if violations:
            print(f"\n❌ {file_path}:", file=sys.stderr)
            for line_num, line_content in violations:
                print(f"  Line {line_num}: {line_content}", file=sys.stderr)
                total_violations += 1

    if total_violations > 0:
        print(
            f"\n{'='*70}\n"
            f"❌ Found {total_violations} generic exception handler(s) in backend code.\n"
            f"\n"
            f"Please use specific exception types (IOError, OSError, ValueError, etc.).\n"
            f"If absolutely necessary, add '# noqa: generic-exception'.\n"
            f"{'='*70}\n",
            file=sys.stderr,
        )
        return 1

    return 0


if __name__ == "__main__":
    sys.exit(main())
