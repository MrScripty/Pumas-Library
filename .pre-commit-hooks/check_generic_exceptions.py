#!/usr/bin/env python3
"""
Pre-commit hook to detect generic exception handlers in backend code.

This enforces the use of specific exception types for clearer error handling.
"""

import logging
import re
import sys
from pathlib import Path

logger = logging.getLogger("check_generic_exceptions")
if not logger.handlers:
    logging.basicConfig(level=logging.ERROR)

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
    except OSError as exc:
        logger.error("Error reading %s: %s", file_path, exc)
        return []

    return violations


def main() -> int:
    """
    Main entry point for the pre-commit hook.

    Returns:
        0 if no violations found, 1 otherwise
    """
    if len(sys.argv) < 2:
        logger.error("Usage: check_generic_exceptions.py <file1> [file2] ...")
        return 1

    files_to_check = [Path(f) for f in sys.argv[1:]]
    total_violations = 0

    for file_path in files_to_check:
        if file_path.suffix != ".py":
            continue

        violations = check_file(file_path)
        if violations:
            logger.error("")
            logger.error("❌ %s:", file_path)
            for line_num, line_content in violations:
                logger.error("  Line %s: %s", line_num, line_content)
                total_violations += 1

    if total_violations > 0:
        logger.error(
            "\n%s\n"
            "❌ Found %s generic exception handler(s) in backend code.\n"
            "\n"
            "Please use specific exception types (IOError, OSError, ValueError, etc.).\n"
            "If absolutely necessary, add '# noqa: generic-exception'.\n"
            "%s\n",
            "=" * 70,
            total_violations,
            "=" * 70,
        )
        return 1

    return 0


if __name__ == "__main__":
    sys.exit(main())
