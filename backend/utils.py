#!/usr/bin/env python3
"""
Utility functions for ComfyUI Version Manager
Path resolution, validation, and helper functions
"""

import hashlib
import subprocess
from pathlib import Path
from typing import Dict, List, Optional


def get_launcher_root() -> Path:
    """
    Get the launcher root directory

    Returns:
        Path to the launcher root directory
    """
    # This should be called from backend/, so parent is the launcher root
    return Path(__file__).parent.parent.resolve()


def ensure_directory(path: Path) -> bool:
    """
    Ensure a directory exists, creating it if necessary

    Args:
        path: Directory path to ensure

    Returns:
        True if directory exists or was created
    """
    try:
        path.mkdir(parents=True, exist_ok=True)
        return True
    except OSError as e:
        print(f"Error creating directory {path}: {e}")
        return False


def calculate_file_hash(file_path: Path, algorithm: str = "sha256") -> Optional[str]:
    """
    Calculate hash of a file

    Args:
        file_path: Path to file
        algorithm: Hash algorithm (sha256, md5, etc.)

    Returns:
        Hex digest of hash or None on error
    """
    try:
        hasher = hashlib.new(algorithm)
        with open(file_path, "rb") as f:
            # Read in chunks to handle large files
            for chunk in iter(lambda: f.read(65536), b""):
                hasher.update(chunk)
        return hasher.hexdigest()
    except (IOError, OSError) as e:
        print(f"Error calculating hash for {file_path}: {e}")
        return None


def calculate_string_hash(content: str, algorithm: str = "sha256") -> str:
    """
    Calculate hash of a string

    Args:
        content: String content to hash
        algorithm: Hash algorithm

    Returns:
        Hex digest of hash
    """
    hasher = hashlib.new(algorithm)
    hasher.update(content.encode("utf-8"))
    return hasher.hexdigest()


def get_directory_size(path: Path) -> int:
    """
    Calculate total size of a directory recursively

    Args:
        path: Directory path

    Returns:
        Total size in bytes
    """
    total = 0
    try:
        for item in path.rglob("*"):
            if item.is_file():
                total += item.stat().st_size
    except (OSError, PermissionError) as e:
        print(f"Error calculating directory size for {path}: {e}")
    return total


def is_valid_symlink(link: Path) -> bool:
    """
    Check if a path is a valid (not broken) symlink

    Args:
        link: Path to check

    Returns:
        True if path is a symlink and target exists
    """
    return link.is_symlink() and link.exists()


def is_broken_symlink(link: Path) -> bool:
    """
    Check if a path is a broken symlink

    Args:
        link: Path to check

    Returns:
        True if path is a symlink but target doesn't exist
    """
    return link.is_symlink() and not link.exists()


def make_relative_symlink(target: Path, link: Path) -> bool:
    """
    Create a relative symlink

    Args:
        target: Symlink target (actual file/directory)
        link: Symlink path (where the link will be created)

    Returns:
        True if successful
    """
    try:
        # Calculate relative path from link to target
        relative_target = Path(relative_path(link.parent, target))

        # Remove existing link if present
        if link.exists() or link.is_symlink():
            link.unlink()

        # Create parent directory if needed
        link.parent.mkdir(parents=True, exist_ok=True)

        # Create symlink
        link.symlink_to(relative_target)
        return True
    except (OSError, ValueError) as e:
        print(f"Error creating symlink {link} -> {target}: {e}")
        return False


def relative_path(from_path: Path, to_path: Path) -> str:
    """
    Calculate relative path from one directory to another

    Args:
        from_path: Starting directory
        to_path: Target path

    Returns:
        Relative path as string
    """
    try:
        # Get absolute paths
        from_abs = from_path.resolve()
        to_abs = to_path.resolve()

        # Calculate relative path
        rel = Path(to_abs).relative_to(from_abs.parent)
        return str(rel)
    except ValueError:
        # If paths don't share a common base, find common ancestor
        from_parts = list(from_abs.parts)
        to_parts = list(to_abs.parts)

        # Find common prefix
        common_length = 0
        for i, (f, t) in enumerate(zip(from_parts, to_parts)):
            if f == t:
                common_length = i + 1
            else:
                break

        # Calculate ".." jumps needed
        up_count = len(from_parts) - common_length
        rel_parts = [".."] * up_count + list(to_parts[common_length:])

        return str(Path(*rel_parts))


def run_command(
    cmd: List[str],
    cwd: Optional[Path] = None,
    timeout: Optional[int] = None,
    env: Optional[Dict[str, str]] = None,
) -> tuple[bool, str, str]:
    """
    Run a shell command and capture output

    Args:
        cmd: Command and arguments as list
        cwd: Working directory
        timeout: Timeout in seconds
        env: Optional environment variables to use

    Returns:
        Tuple of (success, stdout, stderr)
    """
    try:
        result = subprocess.run(
            cmd,
            cwd=str(cwd) if cwd else None,
            capture_output=True,
            text=True,
            timeout=timeout,
            env=env,
        )
        return (result.returncode == 0, result.stdout, result.stderr)
    except subprocess.TimeoutExpired:
        return (False, "", "Command timed out")
    except Exception as e:
        return (False, "", str(e))


def check_command_exists(command: str) -> bool:
    """
    Check if a command is available in PATH

    Args:
        command: Command name to check

    Returns:
        True if command exists
    """
    try:
        result = subprocess.run(["which", command], capture_output=True, text=True)
        return result.returncode == 0
    except Exception:
        return False


def safe_filename(name: str) -> str:
    """
    Convert a string to a safe filename

    Args:
        name: Original name

    Returns:
        Safe filename (alphanumeric, dash, underscore only)
    """
    import re

    # Replace unsafe characters with dash
    safe = re.sub(r"[^a-zA-Z0-9._-]", "-", name)
    # Remove multiple consecutive dashes
    safe = re.sub(r"-+", "-", safe)
    # Strip leading/trailing dashes
    safe = safe.strip("-")
    return safe or "unnamed"


def parse_requirements_file(requirements_path: Path) -> dict[str, str]:
    """
    Parse a requirements.txt file into package->version mapping

    Args:
        requirements_path: Path to requirements.txt

    Returns:
        Dict mapping package names to version specifiers
    """
    requirements = {}

    if not requirements_path.exists():
        return requirements

    try:
        with open(requirements_path, "r") as f:
            for line in f:
                # Strip comments and whitespace
                line = line.split("#")[0].strip()

                # Skip empty lines
                if not line:
                    continue

                # Skip pip flags (lines starting with -)
                if line.startswith("-"):
                    continue

                # Parse package spec
                # Handle formats like: package, package==1.0, package>=1.0,<2.0
                from packaging.requirements import Requirement

                try:
                    req = Requirement(line)
                    # Get package name and version specifier
                    package_name = req.name
                    version_spec = str(req.specifier) if req.specifier else ""
                    requirements[package_name] = version_spec
                except Exception:
                    # If parsing fails, try simple split
                    if "==" in line:
                        package, version = line.split("==", 1)
                        requirements[package.strip()] = f"=={version.strip()}"
                    elif ">=" in line or "<=" in line or ">" in line or "<" in line:
                        # Complex version specifier, store as-is
                        package = line.split(">")[0].split("<")[0].split("=")[0].strip()
                        requirements[package] = line[len(package) :].strip()
                    else:
                        # No version specifier
                        requirements[line.strip()] = ""
    except Exception as e:
        print(f"Error parsing requirements file {requirements_path}: {e}")

    return requirements


def find_files_by_extension(directory: Path, extension: str) -> List[Path]:
    """
    Find all files with a specific extension in a directory recursively

    Args:
        directory: Directory to search
        extension: File extension (with or without leading dot)

    Returns:
        List of matching file paths
    """
    if not extension.startswith("."):
        extension = "." + extension

    try:
        return list(directory.rglob(f"*{extension}"))
    except Exception as e:
        print(f"Error searching for {extension} files in {directory}: {e}")
        return []
