#!/usr/bin/env python3
"""
Pre-commit hook to enforce precise exception handling and logging.

Rules:
- No bare `except:`
- No `except Exception` / `except BaseException`
- Only one exception type per `except` clause
- Every `except` block must log using logger.* or logging.*
"""

import ast
import logging
import sys
from pathlib import Path

logger = logging.getLogger("check_generic_exceptions")
if not logger.handlers:
    logging.basicConfig(level=logging.ERROR)

GENERIC_EXCEPTION_NAMES = {"Exception", "BaseException"}
LOG_METHODS = {"debug", "info", "warning", "error", "exception", "critical"}
LOG_BASE_NAMES = {"logger", "logging"}
NOQA_GENERIC = "noqa: generic-exception"
NOQA_MULTI = "noqa: multi-exception"
NOQA_LOGGING = "noqa: no-except-logging"


def has_noqa(lines: list[str], start_line: int, end_line: int, token: str) -> bool:
    for line in lines[start_line - 1 : end_line]:
        if token in line:
            return True
    return False


def is_generic_exception(type_node: ast.AST) -> bool:
    if isinstance(type_node, ast.Name):
        return type_node.id in GENERIC_EXCEPTION_NAMES
    if isinstance(type_node, ast.Attribute):
        return type_node.attr in GENERIC_EXCEPTION_NAMES
    return False


def is_multi_exception_tuple(type_node: ast.AST) -> bool:
    return isinstance(type_node, ast.Tuple) and len(type_node.elts) > 1


def is_raise_only(handler: ast.ExceptHandler) -> bool:
    return bool(handler.body) and all(isinstance(stmt, ast.Raise) for stmt in handler.body)


def has_logging_call(handler: ast.ExceptHandler) -> bool:
    module = ast.Module(body=handler.body, type_ignores=[])
    for node in ast.walk(module):
        if not isinstance(node, ast.Call):
            continue
        func = node.func
        if not isinstance(func, ast.Attribute):
            continue
        if func.attr not in LOG_METHODS:
            continue
        if isinstance(func.value, ast.Name) and func.value.id in LOG_BASE_NAMES:
            return True
        if isinstance(func.value, ast.Attribute) and func.value.attr == "logger":
            return True
    return False


def check_file(file_path: Path) -> list[tuple[int, str, str]]:
    """
    Check a file for exception handling violations.

    Args:
        file_path: Path to the Python file to check

    Returns:
        List of (line_number, message, line_content) tuples for violations
    """
    violations: list[tuple[int, str, str]] = []

    try:
        text = file_path.read_text(encoding="utf-8")
    except OSError as exc:
        logger.error("Error reading %s: %s", file_path, exc)
        return []

    try:
        tree = ast.parse(text, filename=str(file_path))
    except SyntaxError as exc:
        logger.error("Error parsing %s: %s", file_path, exc)
        return []

    lines = text.splitlines()

    for node in ast.walk(tree):
        if not isinstance(node, ast.Try):
            continue
        for handler in node.handlers:
            line_num = handler.lineno
            end_line = getattr(handler, "end_lineno", line_num)
            line_content = lines[line_num - 1].rstrip() if line_num <= len(lines) else ""

            if handler.type is None:
                if not has_noqa(lines, line_num, end_line, NOQA_GENERIC):
                    violations.append((line_num, "Bare except is not allowed", line_content))
                continue

            if is_generic_exception(handler.type):
                if not has_noqa(lines, line_num, end_line, NOQA_GENERIC):
                    violations.append(
                        (line_num, "Generic exception handler is not allowed", line_content)
                    )

            if is_multi_exception_tuple(handler.type):
                if not has_noqa(lines, line_num, end_line, NOQA_MULTI):
                    violations.append(
                        (line_num, "Multiple exception types are not allowed", line_content)
                    )

            if not has_noqa(lines, line_num, end_line, NOQA_LOGGING):
                if is_raise_only(handler):
                    continue
                if not has_logging_call(handler):
                    violations.append((line_num, "Except block missing logger call", line_content))

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
            for line_num, message, line_content in violations:
                logger.error("  Line %s: %s | %s", line_num, message, line_content)
                total_violations += 1

    if total_violations > 0:
        logger.error(
            "\n%s\n"
            "❌ Found %s exception handling violation(s).\n"
            "\n"
            "Please use specific exception types (no tuples), and log in every except.\n"
            "Raise-only except blocks may omit logging.\n"
            "If absolutely necessary, add '# noqa: generic-exception' or "
            "'# noqa: multi-exception' or '# noqa: no-except-logging'.\n"
            "%s\n",
            "=" * 70,
            total_violations,
            "=" * 70,
        )
        return 1

    return 0


if __name__ == "__main__":
    sys.exit(main())
