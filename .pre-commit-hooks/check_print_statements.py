#!/usr/bin/env python3
"""
Pre-commit hook to detect print statements in Python code.

This enforces the use of the logging system instead of print for better
troubleshooting and monitoring in production.

Exceptions:
- Lines with 'noqa: print' comment are allowed
- User-facing console output functions (explicitly marked)
- Test files that need to verify print behavior
"""

import logging
import re
import sys
from pathlib import Path

logger = logging.getLogger(__name__)


def check_file(file_path: Path) -> list[tuple[int, str]]:
    """
    Check a file for print statements.

    Args:
        file_path: Path to the Python file to check

    Returns:
        List of (line_number, line_content) tuples for violations
    """
    violations = []

    try:
        with open(file_path, "r", encoding="utf-8") as f:
            for line_num, line in enumerate(f, start=1):
                # Skip empty lines and comments
                stripped = line.strip()
                if not stripped or stripped.startswith("#"):
                    continue

                # Check for print() calls
                if re.search(r"\bprint\s*\(", line):
                    # Allow if explicitly marked with noqa comment
                    if "noqa: print" in line or "noqa:print" in line:
                        continue

                    # Allow in main blocks that are for testing
                    # (we check context in next iteration if needed)

                    violations.append((line_num, line.rstrip()))

    except OSError as e:
        logger.warning("Failed to read %s: %s", file_path, e, exc_info=True)
        sys.stderr.write(f"Error reading {file_path}: {e}\n")
        return []

    return violations


def main() -> int:
    """
    Main entry point for the pre-commit hook.

    Returns:
        0 if no violations found, 1 otherwise
    """
    if len(sys.argv) < 2:
        sys.stderr.write("Usage: check_print_statements.py <file1> [file2] ...\n")
        return 1

    files_to_check = [Path(f) for f in sys.argv[1:]]
    total_violations = 0

    for file_path in files_to_check:
        if file_path.suffix != ".py":
            continue

        violations = check_file(file_path)
        if violations:
            sys.stderr.write(f"\n❌ {file_path}:\n")
            for line_num, line_content in violations:
                sys.stderr.write(f"  Line {line_num}: {line_content}\n")
                total_violations += 1

    if total_violations > 0:
        sys.stderr.write(
            f"\n{'='*70}\n"
            f"❌ Found {total_violations} print statement(s) in Python code.\n"
            f"\n"
            f"Please use the logging system instead:\n"
            f"  from backend.logging_config import get_logger\n"
            f"  logger = get_logger(__name__)\n"
            f"  logger.info('message')  # or .debug, .warning, .error\n"
            f"\n"
            f"If this is intentional user-facing output, add '# noqa: print'\n"
            f"{'='*70}\n"
        )
        return 1

    return 0


if __name__ == "__main__":
    sys.exit(main())
