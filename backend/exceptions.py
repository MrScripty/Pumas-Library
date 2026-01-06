"""
Custom exceptions for the ComfyUI Launcher.

This module defines a hierarchy of exceptions to replace generic Exception catching
throughout the codebase. This allows for more precise error handling and better
debugging by distinguishing between different types of failures.
"""

from typing import Optional


class ComfyUILauncherError(Exception):
    """
    Base exception for all ComfyUI Launcher errors.

    All custom exceptions in this application should inherit from this class.
    This allows catching all launcher-specific errors with a single except clause
    while letting system errors (KeyboardInterrupt, SystemExit) bubble up.
    """

    pass


class InstallationError(ComfyUILauncherError):
    """
    Raised when installation of a ComfyUI version fails.

    This includes failures during:
    - Downloading release archives
    - Extracting archives
    - Setting up the installation directory
    - Post-installation setup steps

    Args:
        message: Human-readable error description
        version_tag: The version that failed to install (optional)
    """

    def __init__(self, message: str, version_tag: Optional[str] = None):
        self.version_tag = version_tag
        if version_tag:
            super().__init__(f"{message} (version: {version_tag})")
        else:
            super().__init__(message)


class DependencyError(ComfyUILauncherError):
    """
    Raised when dependency resolution or installation fails.

    This includes failures during:
    - Resolving package constraints
    - Querying PyPI for package versions
    - Installing packages via pip
    - Creating or activating virtual environments

    Args:
        message: Human-readable error description
        package_name: The problematic package (optional)
    """

    def __init__(self, message: str, package_name: Optional[str] = None):
        self.package_name = package_name
        if package_name:
            super().__init__(f"{message} (package: {package_name})")
        else:
            super().__init__(message)


class NetworkError(ComfyUILauncherError):
    """
    Raised when network operations fail.

    This includes failures during:
    - GitHub API requests (rate limits, connection errors, timeouts)
    - Downloading files from remote servers
    - DNS resolution failures
    - SSL/TLS errors

    Args:
        message: Human-readable error description
        url: The URL that failed (optional)
        status_code: HTTP status code if applicable (optional)
    """

    def __init__(self, message: str, url: Optional[str] = None, status_code: Optional[int] = None):
        self.url = url
        self.status_code = status_code
        error_parts = [message]
        if url:
            error_parts.append(f"URL: {url}")
        if status_code:
            error_parts.append(f"Status: {status_code}")
        super().__init__(" | ".join(error_parts))


class ValidationError(ComfyUILauncherError):
    """
    Raised when input validation fails.

    This includes failures during:
    - Version tag validation (invalid characters, format)
    - Path validation (path traversal attempts, invalid paths)
    - URL validation (invalid schemes, malformed URLs)
    - Package name validation

    Args:
        message: Human-readable error description
        field_name: The field that failed validation (optional)
        invalid_value: The value that failed validation (optional)
    """

    def __init__(
        self,
        message: str,
        field_name: Optional[str] = None,
        invalid_value: Optional[str] = None,
    ):
        self.field_name = field_name
        self.invalid_value = invalid_value
        error_parts = [message]
        if field_name:
            error_parts.append(f"field: {field_name}")
        if invalid_value:
            error_parts.append(f"value: {invalid_value}")
        super().__init__(" | ".join(error_parts))


class MetadataError(ComfyUILauncherError):
    """
    Raised when metadata operations fail.

    This includes failures during:
    - Reading metadata files
    - Writing metadata files
    - Parsing JSON metadata
    - Metadata validation
    - Corrupted metadata detection

    Args:
        message: Human-readable error description
        file_path: The metadata file that caused the error (optional)
    """

    def __init__(self, message: str, file_path: Optional[str] = None):
        self.file_path = file_path
        if file_path:
            super().__init__(f"{message} (file: {file_path})")
        else:
            super().__init__(message)


class ProcessError(ComfyUILauncherError):
    """
    Raised when subprocess operations fail.

    This includes failures during:
    - Launching ComfyUI server process
    - Subprocess crashes or unexpected exits
    - Process health check failures
    - Process communication errors

    Args:
        message: Human-readable error description
        exit_code: Process exit code (optional)
        command: The command that failed (optional)
    """

    def __init__(
        self,
        message: str,
        exit_code: Optional[int] = None,
        command: Optional[str] = None,
    ):
        self.exit_code = exit_code
        self.command = command
        error_parts = [message]
        if command:
            error_parts.append(f"command: {command}")
        if exit_code is not None:
            error_parts.append(f"exit code: {exit_code}")
        super().__init__(" | ".join(error_parts))


class ResourceError(ComfyUILauncherError):
    """
    Raised when resource management operations fail.

    This includes failures during:
    - Checking disk space
    - File I/O operations
    - Directory creation or removal
    - Permission errors
    - Resource cleanup failures

    Args:
        message: Human-readable error description
        resource_type: Type of resource (disk, memory, file, etc.)
    """

    def __init__(self, message: str, resource_type: Optional[str] = None):
        self.resource_type = resource_type
        if resource_type:
            super().__init__(f"{message} (resource: {resource_type})")
        else:
            super().__init__(message)


class CancellationError(ComfyUILauncherError):
    """
    Raised when an operation is cancelled by the user.

    This is distinct from a failure - the operation was intentionally stopped.
    Use this to distinguish user-initiated cancellations from errors.

    Args:
        message: Human-readable description of what was cancelled
    """

    pass
