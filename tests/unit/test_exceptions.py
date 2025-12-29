"""
Unit tests for custom exception classes.

Tests verify:
- Exception hierarchy and inheritance
- Exception message formatting with optional parameters
- Exception attributes are properly set
- Exceptions can be raised and caught correctly
"""

import pytest

from backend.exceptions import (
    CancellationError,
    ComfyUILauncherError,
    DependencyError,
    InstallationError,
    MetadataError,
    NetworkError,
    ProcessError,
    ResourceError,
    ValidationError,
)


class TestComfyUILauncherError:
    """Tests for the base exception class."""

    def test_base_exception_can_be_raised(self):
        """Base exception can be instantiated and raised."""
        with pytest.raises(ComfyUILauncherError) as exc_info:
            raise ComfyUILauncherError("Test error")

        assert str(exc_info.value) == "Test error"

    def test_base_exception_is_exception_subclass(self):
        """Base exception inherits from built-in Exception."""
        assert issubclass(ComfyUILauncherError, Exception)

    def test_can_catch_all_launcher_errors(self):
        """All custom exceptions can be caught with base exception."""
        custom_exceptions = [
            InstallationError("test"),
            DependencyError("test"),
            NetworkError("test"),
            ValidationError("test"),
            MetadataError("test"),
            ProcessError("test"),
            ResourceError("test"),
            CancellationError("test"),
        ]

        for exc in custom_exceptions:
            with pytest.raises(ComfyUILauncherError):
                raise exc


class TestInstallationError:
    """Tests for InstallationError."""

    def test_simple_message(self):
        """InstallationError with simple message."""
        error = InstallationError("Installation failed")
        assert str(error) == "Installation failed"
        assert error.version_tag is None

    def test_with_version_tag(self):
        """InstallationError with version tag."""
        error = InstallationError("Installation failed", version_tag="v0.5.1")
        assert str(error) == "Installation failed (version: v0.5.1)"
        assert error.version_tag == "v0.5.1"

    def test_inherits_from_base(self):
        """InstallationError inherits from ComfyUILauncherError."""
        assert issubclass(InstallationError, ComfyUILauncherError)

    def test_can_be_caught_specifically(self):
        """InstallationError can be caught with specific except clause."""
        with pytest.raises(InstallationError) as exc_info:
            raise InstallationError("Download failed", version_tag="v1.0.0")

        assert exc_info.value.version_tag == "v1.0.0"


class TestDependencyError:
    """Tests for DependencyError."""

    def test_simple_message(self):
        """DependencyError with simple message."""
        error = DependencyError("Dependency resolution failed")
        assert str(error) == "Dependency resolution failed"
        assert error.package_name is None

    def test_with_package_name(self):
        """DependencyError with package name."""
        error = DependencyError("Package not found", package_name="torch")
        assert str(error) == "Package not found (package: torch)"
        assert error.package_name == "torch"

    def test_inherits_from_base(self):
        """DependencyError inherits from ComfyUILauncherError."""
        assert issubclass(DependencyError, ComfyUILauncherError)


class TestNetworkError:
    """Tests for NetworkError."""

    def test_simple_message(self):
        """NetworkError with simple message."""
        error = NetworkError("Connection failed")
        assert str(error) == "Connection failed"
        assert error.url is None
        assert error.status_code is None

    def test_with_url_only(self):
        """NetworkError with URL."""
        error = NetworkError("Request failed", url="https://example.com")
        assert str(error) == "Request failed | URL: https://example.com"
        assert error.url == "https://example.com"
        assert error.status_code is None

    def test_with_status_code_only(self):
        """NetworkError with status code."""
        error = NetworkError("HTTP error", status_code=404)
        assert str(error) == "HTTP error | Status: 404"
        assert error.url is None
        assert error.status_code == 404

    def test_with_url_and_status_code(self):
        """NetworkError with both URL and status code."""
        error = NetworkError("Request failed", url="https://api.github.com", status_code=403)
        assert str(error) == "Request failed | URL: https://api.github.com | Status: 403"
        assert error.url == "https://api.github.com"
        assert error.status_code == 403

    def test_inherits_from_base(self):
        """NetworkError inherits from ComfyUILauncherError."""
        assert issubclass(NetworkError, ComfyUILauncherError)


class TestValidationError:
    """Tests for ValidationError."""

    def test_simple_message(self):
        """ValidationError with simple message."""
        error = ValidationError("Invalid input")
        assert str(error) == "Invalid input"
        assert error.field_name is None
        assert error.invalid_value is None

    def test_with_field_name_only(self):
        """ValidationError with field name."""
        error = ValidationError("Invalid format", field_name="version_tag")
        assert str(error) == "Invalid format | field: version_tag"
        assert error.field_name == "version_tag"
        assert error.invalid_value is None

    def test_with_invalid_value_only(self):
        """ValidationError with invalid value."""
        error = ValidationError("Invalid characters", invalid_value="../etc/passwd")
        assert str(error) == "Invalid characters | value: ../etc/passwd"
        assert error.field_name is None
        assert error.invalid_value == "../etc/passwd"

    def test_with_field_and_value(self):
        """ValidationError with both field name and invalid value."""
        error = ValidationError(
            "Invalid URL", field_name="download_url", invalid_value="ftp://bad.com"
        )
        assert str(error) == "Invalid URL | field: download_url | value: ftp://bad.com"
        assert error.field_name == "download_url"
        assert error.invalid_value == "ftp://bad.com"

    def test_inherits_from_base(self):
        """ValidationError inherits from ComfyUILauncherError."""
        assert issubclass(ValidationError, ComfyUILauncherError)


class TestMetadataError:
    """Tests for MetadataError."""

    def test_simple_message(self):
        """MetadataError with simple message."""
        error = MetadataError("Metadata corrupted")
        assert str(error) == "Metadata corrupted"
        assert error.file_path is None

    def test_with_file_path(self):
        """MetadataError with file path."""
        error = MetadataError("Failed to parse JSON", file_path="/path/to/metadata.json")
        assert str(error) == "Failed to parse JSON (file: /path/to/metadata.json)"
        assert error.file_path == "/path/to/metadata.json"

    def test_inherits_from_base(self):
        """MetadataError inherits from ComfyUILauncherError."""
        assert issubclass(MetadataError, ComfyUILauncherError)


class TestProcessError:
    """Tests for ProcessError."""

    def test_simple_message(self):
        """ProcessError with simple message."""
        error = ProcessError("Process crashed")
        assert str(error) == "Process crashed"
        assert error.exit_code is None
        assert error.command is None

    def test_with_exit_code_only(self):
        """ProcessError with exit code."""
        error = ProcessError("Process failed", exit_code=1)
        assert str(error) == "Process failed | exit code: 1"
        assert error.exit_code == 1
        assert error.command is None

    def test_with_command_only(self):
        """ProcessError with command."""
        error = ProcessError("Command failed", command="python main.py")
        assert str(error) == "Command failed | command: python main.py"
        assert error.exit_code is None
        assert error.command == "python main.py"

    def test_with_command_and_exit_code(self):
        """ProcessError with both command and exit code."""
        error = ProcessError("Execution failed", exit_code=137, command="./server.py")
        assert str(error) == "Execution failed | command: ./server.py | exit code: 137"
        assert error.exit_code == 137
        assert error.command == "./server.py"

    def test_exit_code_zero(self):
        """ProcessError correctly handles exit code 0."""
        error = ProcessError("Unexpected success", exit_code=0)
        assert "exit code: 0" in str(error)
        assert error.exit_code == 0

    def test_inherits_from_base(self):
        """ProcessError inherits from ComfyUILauncherError."""
        assert issubclass(ProcessError, ComfyUILauncherError)


class TestResourceError:
    """Tests for ResourceError."""

    def test_simple_message(self):
        """ResourceError with simple message."""
        error = ResourceError("Resource unavailable")
        assert str(error) == "Resource unavailable"
        assert error.resource_type is None

    def test_with_resource_type(self):
        """ResourceError with resource type."""
        error = ResourceError("Insufficient space", resource_type="disk")
        assert str(error) == "Insufficient space (resource: disk)"
        assert error.resource_type == "disk"

    def test_inherits_from_base(self):
        """ResourceError inherits from ComfyUILauncherError."""
        assert issubclass(ResourceError, ComfyUILauncherError)


class TestCancellationError:
    """Tests for CancellationError."""

    def test_simple_message(self):
        """CancellationError with simple message."""
        error = CancellationError("Operation cancelled by user")
        assert str(error) == "Operation cancelled by user"

    def test_inherits_from_base(self):
        """CancellationError inherits from ComfyUILauncherError."""
        assert issubclass(CancellationError, ComfyUILauncherError)

    def test_distinct_from_other_errors(self):
        """CancellationError represents user action, not failure."""
        with pytest.raises(CancellationError) as exc_info:
            raise CancellationError("User clicked cancel")

        # Should not be catchable by other specific error types
        assert not isinstance(exc_info.value, InstallationError)
        assert not isinstance(exc_info.value, NetworkError)

        # But should be catchable by base exception
        assert isinstance(exc_info.value, ComfyUILauncherError)


class TestExceptionChaining:
    """Tests for exception chaining (raise ... from ...)."""

    def test_can_chain_exceptions(self):
        """Custom exceptions support exception chaining."""
        try:
            try:
                raise ValueError("Original error")
            except ValueError as e:
                raise NetworkError("GitHub API failed") from e
        except NetworkError as exc_info:
            assert exc_info.__cause__.__class__.__name__ == "ValueError"
            assert str(exc_info.__cause__) == "Original error"

    def test_chained_exceptions_preserve_traceback(self):
        """Chained exceptions preserve original traceback."""
        try:
            try:
                # Simulate deep call stack
                def inner():
                    raise FileNotFoundError("metadata.json not found")

                inner()
            except FileNotFoundError as e:
                raise MetadataError("Failed to load metadata") from e
        except MetadataError as exc:
            assert exc.__cause__ is not None
            assert isinstance(exc.__cause__, FileNotFoundError)
