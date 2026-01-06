#!/usr/bin/env python3
"""
Pre-commit hook to prevent hardcoded colors in frontend code.
Enforces the use of CSS theme variables instead of hardcoded hex/rgb values.
"""

import re
import sys
from pathlib import Path
from typing import List, Tuple

# ANSI color codes
RED = "\033[0;31m"
YELLOW = "\033[1;33m"
GREEN = "\033[0;32m"
BOLD = "\033[1m"
NC = "\033[0m"  # No Color


# Patterns to detect hardcoded colors
PATTERNS = [
    # Hex colors in Tailwind classes: bg-[#2a2a2a], text-[#fff]
    (
        r"(bg|text|border)-\[#[0-9a-fA-F]{3,8}\]",
        "Hex color in Tailwind class",
    ),
    # Hex colors in strings/props: "#2a2a2a", '#fff'
    (
        r"""['"](#[0-9a-fA-F]{3,8})['"]""",
        "Hex color string",
    ),
    # RGB/RGBA in Tailwind: bg-[rgb(42,42,42)], text-[rgba(255,255,255,0.5)]
    (
        r"(bg|text|border)-\[rgba?\([0-9,\s.]+\)\]",
        "RGB/RGBA color in Tailwind class",
    ),
    # Hardcoded Tailwind color utilities: text-gray-500, bg-white, border-red-500
    (
        r"(bg|text|border)-(gray|white|black|red|green|blue|yellow|orange|purple|pink|indigo|teal|cyan)-[0-9]+",
        "Hardcoded Tailwind color class",
    ),
]

# Exceptions - lines that should be ignored
EXCEPTION_PATTERNS = [
    r"^\s*//",  # Single-line comments
    r"^\s*\*",  # Multi-line comments
    r"import\s",  # Import statements
    r"from\s",  # From imports
    r"theme\.ts",  # Theme configuration file
    r"index\.css",  # CSS file
    r"deleteZone",  # Delete zone overlay (intentionally hardcoded red)
    r"text-transparent",  # Transparent is allowed
    r"bg-transparent",  # Transparent is allowed
    r"border-transparent",  # Transparent is allowed
]

# Files to exclude entirely
EXCLUDED_FILES = [
    "frontend/src/config/theme.ts",
    "frontend/src/index.css",
]


def should_ignore_line(line: str) -> bool:
    """Check if a line should be ignored based on exception patterns."""
    return any(re.search(pattern, line) for pattern in EXCEPTION_PATTERNS)


def check_file(file_path: Path) -> List[Tuple[int, str, str]]:
    """
    Check a file for hardcoded colors.
    Returns list of (line_number, matched_text, pattern_description).
    """
    # Skip excluded files
    if str(file_path) in EXCLUDED_FILES or any(excl in str(file_path) for excl in EXCLUDED_FILES):
        return []

    try:
        with open(file_path, "r", encoding="utf-8") as f:
            lines = f.readlines()
    except (OSError, UnicodeError) as e:
        sys.stderr.write(f"Error reading {file_path}: {e}\n")
        return []

    violations = []

    for line_num, line in enumerate(lines, start=1):
        # Skip ignored lines
        if should_ignore_line(line):
            continue

        # Check each pattern
        for pattern, description in PATTERNS:
            matches = re.finditer(pattern, line)
            for match in matches:
                violations.append((line_num, match.group(0), description))

    return violations


def write_out(message: str) -> None:
    sys.stdout.write(f"{message}\n")


def main():
    """Main entry point for the pre-commit hook."""
    # Get list of staged files from command line arguments
    if len(sys.argv) < 2:
        # No files provided
        return 0

    files_to_check = [
        Path(f)
        for f in sys.argv[1:]
        if f.endswith((".ts", ".tsx", ".js", ".jsx")) and "frontend/src" in f
    ]

    if not files_to_check:
        # No relevant files to check
        return 0

    write_out(f"{BOLD}🎨 Checking for hardcoded colors in staged files...{NC}")

    all_violations = {}
    for file_path in files_to_check:
        violations = check_file(file_path)
        if violations:
            all_violations[file_path] = violations

    if all_violations:
        write_out("")
        write_out(f"{RED}{BOLD}❌ Hardcoded colors detected!{NC}")
        write_out("")

        for file_path, violations in all_violations.items():
            write_out(f"{YELLOW}File: {file_path}{NC}")
            for line_num, matched_text, description in violations:
                write_out(f"  Line {line_num}: {matched_text} ({description})")
            write_out("")

        write_out(f"{RED}{'━' * 60}{NC}")
        write_out(f"{RED}{BOLD}Commit rejected: Hardcoded colors detected{NC}")
        write_out(f"{RED}{'━' * 60}{NC}")
        write_out("")
        write_out("Please use theme variables instead of hardcoded colors:")
        write_out("")
        write_out(f"  {RED}❌ Bad:{NC}  bg-[#2a2a2a]")
        write_out(f"  {GREEN}✅ Good:{NC} bg-[hsl(var(--surface-interactive))]")
        write_out("")
        write_out(f"  {RED}❌ Bad:{NC}  text-gray-500")
        write_out(f"  {GREEN}✅ Good:{NC} text-[hsl(var(--text-tertiary))]")
        write_out("")
        write_out(f'  {RED}❌ Bad:{NC}  className="text-[#55ff55]"')
        write_out(f'  {GREEN}✅ Good:{NC} className="text-[hsl(var(--accent-success))]"')
        write_out("")
        write_out("Or use utility classes:")
        write_out(f"  {GREEN}✅ Good:{NC} surface-interactive")
        write_out(f"  {GREEN}✅ Good:{NC} text-tertiary")
        write_out(f"  {GREEN}✅ Good:{NC} text-accent-success")
        write_out("")
        write_out("See frontend/THEME_SYSTEM.md for more information.")
        write_out("")

        return 1

    write_out(f"{GREEN}✅ No hardcoded colors found. Commit allowed.{NC}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
