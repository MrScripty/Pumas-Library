#!/usr/bin/env python3
"""
Pre-commit hook to enforce incremental test coverage.

This hook ensures that newly created or modified Python files in the backend/
directory have at least 80% test coverage, while not blocking commits for
existing files that don't yet meet the coverage threshold.

Usage:
    Called automatically by pre-commit on staged files.

Exit codes:
    0: All staged files meet coverage requirements
    1: One or more staged files have insufficient coverage
"""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path


def get_staged_python_files() -> list[str]:
    """Get list of staged Python files in backend/ directory."""
    try:
        result = subprocess.run(
            ["git", "diff", "--cached", "--name-only", "--diff-filter=ACM"],
            capture_output=True,
            text=True,
            check=True,
        )
        files = result.stdout.strip().split("\n")
        # Filter for backend Python files only
        backend_files = [f for f in files if f.startswith("backend/") and f.endswith(".py")]
        return backend_files
    except subprocess.CalledProcessError:
        return []


def get_file_coverage(file_path: str) -> float | None:
    """
    Get test coverage percentage for a specific file.

    Returns:
        Coverage percentage (0-100) or None if file not in coverage report
    """
    try:
        # Run pytest with coverage for the specific file
        result = subprocess.run(
            [
                "./venv/bin/python",
                "-m",
                "pytest",
                "--cov=backend",
                "--cov-report=term-missing:skip-covered",
                "--tb=no",
                "-q",
                "--no-cov-on-fail",
            ],
            capture_output=True,
            text=True,
            cwd=Path(__file__).parent.parent,
        )

        # Parse coverage output to find the specific file
        for line in result.stdout.split("\n"):
            if file_path in line:
                # Line format: "backend/file.py    123    45  63.41%  line-numbers"
                parts = line.split()
                for part in parts:
                    if "%" in part:
                        return float(part.rstrip("%"))
        return None
    except (subprocess.CalledProcessError, ValueError):
        return None


def is_new_file(file_path: str) -> bool:
    """Check if a file is newly created (not just modified)."""
    try:
        result = subprocess.run(
            ["git", "diff", "--cached", "--name-status", file_path],
            capture_output=True,
            text=True,
            check=True,
        )
        status = result.stdout.strip().split()[0]
        return status == "A"  # A = Added
    except (subprocess.CalledProcessError, IndexError):
        return False


def check_incremental_coverage(min_coverage: float = 80.0) -> int:
    """
    Check that staged files meet minimum coverage requirements.

    Args:
        min_coverage: Minimum required coverage percentage (default: 80%)

    Returns:
        0 if all files pass, 1 if any file fails
    """
    staged_files = get_staged_python_files()

    if not staged_files:
        # No backend Python files staged, nothing to check
        return 0

    print("🔍 Checking test coverage for staged files...")  # noqa: print
    print(f"   Minimum required coverage: {min_coverage}%")  # noqa: print
    print()  # noqa: print

    failures = []

    for file_path in staged_files:
        is_new = is_new_file(file_path)
        coverage = get_file_coverage(file_path)

        if coverage is None:
            # File not in coverage report (might be excluded or no tests run)
            if is_new:
                print(f"⚠️  {file_path}")  # noqa: print
                print(f"   Status: NEW FILE - No coverage data found")  # noqa: print
                print(f"   Required: {min_coverage}% coverage")  # noqa: print
                failures.append((file_path, 0.0, is_new))
            else:
                # Modified file with no coverage - allow for now (incremental approach)
                print(f"ℹ️  {file_path}")  # noqa: print
                print(f"   Status: MODIFIED - No coverage data (legacy file)")  # noqa: print
                print(  # noqa: print
                    f"   Action: Coverage enforcement skipped (incremental approach)"
                )
        elif coverage < min_coverage:
            if is_new:
                print(f"❌ {file_path}")  # noqa: print
                print(f"   Coverage: {coverage:.2f}% (NEW FILE)")  # noqa: print
                print(f"   Required: {min_coverage}%")  # noqa: print
                failures.append((file_path, coverage, is_new))
            else:
                # Modified file below threshold - warn but don't block
                print(f"⚠️  {file_path}")  # noqa: print
                print(f"   Coverage: {coverage:.2f}% (MODIFIED)")  # noqa: print
                print(  # noqa: print
                    f"   Target: {min_coverage}% (not enforced for existing files)"
                )
                print(f"   Action: Consider adding tests to improve coverage")  # noqa: print
        else:
            print(f"✅ {file_path}")  # noqa: print
            print(f"   Coverage: {coverage:.2f}%")  # noqa: print

        print()  # noqa: print

    if failures:
        print("=" * 70)  # noqa: print
        print("❌ COVERAGE CHECK FAILED")  # noqa: print
        print("=" * 70)  # noqa: print
        print()  # noqa: print
        print("The following NEW files have insufficient test coverage:")  # noqa: print
        print()  # noqa: print
        for file_path, coverage, is_new in failures:
            print(f"  • {file_path}: {coverage:.2f}% (required: {min_coverage}%)")  # noqa: print
        print()  # noqa: print
        print("Please add tests to meet the minimum coverage requirement.")  # noqa: print
        print("Run this command to see which lines need coverage:")  # noqa: print
        print()  # noqa: print
        print(f"  pytest --cov=backend --cov-report=term-missing")  # noqa: print
        print()  # noqa: print
        return 1

    print("=" * 70)  # noqa: print
    print("✅ COVERAGE CHECK PASSED")  # noqa: print
    print("=" * 70)  # noqa: print
    print()  # noqa: print
    return 0


if __name__ == "__main__":
    sys.exit(check_incremental_coverage(min_coverage=80.0))
