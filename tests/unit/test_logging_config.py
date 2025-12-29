"""
Unit tests for backend/logging_config.py

Tests cover:
- Logging setup and initialization
- File handler configuration
- Console handler configuration
- Logger retrieval
- Reset functionality
- Migration helpers
"""

import logging
from pathlib import Path

import pytest

from backend.logging_config import get_logger, log_print_replacement, reset_logging, setup_logging


@pytest.fixture(autouse=True)
def reset_logging_state():
    """Reset logging state before and after each test."""
    reset_logging()
    yield
    reset_logging()


@pytest.fixture
def temp_log_file(tmp_path):
    """Create a temporary log file path."""
    return tmp_path / "test_launcher.log"


class TestLoggingSetup:
    """Tests for setup_logging() function."""

    def test_setup_logging_creates_log_file(self, temp_log_file):
        """Test that setup_logging creates the log file."""
        setup_logging(log_file=temp_log_file)

        assert temp_log_file.exists()
        assert temp_log_file.is_file()

    def test_setup_logging_creates_log_directory(self, tmp_path):
        """Test that setup_logging creates parent directories if needed."""
        log_file = tmp_path / "nested" / "dir" / "launcher.log"

        setup_logging(log_file=log_file)

        assert log_file.parent.exists()
        assert log_file.exists()

    def test_setup_logging_configures_root_logger(self, temp_log_file):
        """Test that setup_logging configures the root logger."""
        setup_logging(log_file=temp_log_file)

        root_logger = logging.getLogger()
        assert root_logger.level == logging.DEBUG
        assert len(root_logger.handlers) == 2  # File + console

    def test_setup_logging_file_handler_level(self, temp_log_file):
        """Test that file handler has correct log level."""
        setup_logging(log_file=temp_log_file, log_level="DEBUG")

        root_logger = logging.getLogger()
        file_handler = root_logger.handlers[0]
        assert file_handler.level == logging.DEBUG

    def test_setup_logging_console_handler_level(self, temp_log_file):
        """Test that console handler has correct log level."""
        setup_logging(log_file=temp_log_file, console_level="ERROR")

        root_logger = logging.getLogger()
        console_handler = root_logger.handlers[1]
        assert console_handler.level == logging.ERROR

    def test_setup_logging_only_once(self, temp_log_file):
        """Test that setup_logging only initializes once."""
        setup_logging(log_file=temp_log_file)
        setup_logging(log_file=temp_log_file)  # Should be ignored

        root_logger = logging.getLogger()
        assert len(root_logger.handlers) == 2  # Still only 2 handlers

    def test_setup_logging_rotation_config(self, temp_log_file):
        """Test that rotating file handler is configured correctly."""
        max_bytes = 5_000_000
        backup_count = 3

        setup_logging(log_file=temp_log_file, max_bytes=max_bytes, backup_count=backup_count)

        root_logger = logging.getLogger()
        file_handler = root_logger.handlers[0]

        assert file_handler.maxBytes == max_bytes
        assert file_handler.backupCount == backup_count

    def test_setup_logging_writes_initialization_message(self, temp_log_file):
        """Test that initialization message is logged."""
        setup_logging(log_file=temp_log_file, log_level="INFO")

        with open(temp_log_file) as f:
            content = f.read()

        assert "Logging initialized" in content
        assert str(temp_log_file) in content
        assert "level=INFO" in content


class TestGetLogger:
    """Tests for get_logger() function."""

    def test_get_logger_returns_logger(self, temp_log_file):
        """Test that get_logger returns a logger instance."""
        setup_logging(log_file=temp_log_file)

        logger = get_logger("test_module")

        assert isinstance(logger, logging.Logger)
        assert logger.name == "test_module"

    def test_get_logger_auto_initializes(self, temp_log_file):
        """Test that get_logger auto-initializes logging if not setup."""
        # Don't call setup_logging()
        logger = get_logger("test_module")

        # Should still work, with default initialization
        assert isinstance(logger, logging.Logger)
        root_logger = logging.getLogger()
        assert len(root_logger.handlers) == 2

    def test_get_logger_same_instance(self, temp_log_file):
        """Test that get_logger returns same instance for same name."""
        setup_logging(log_file=temp_log_file)

        logger1 = get_logger("test_module")
        logger2 = get_logger("test_module")

        assert logger1 is logger2

    def test_get_logger_different_instances(self, temp_log_file):
        """Test that get_logger returns different instances for different names."""
        setup_logging(log_file=temp_log_file)

        logger1 = get_logger("module1")
        logger2 = get_logger("module2")

        assert logger1 is not logger2
        assert logger1.name != logger2.name


class TestLogging:
    """Tests for actual logging functionality."""

    def test_logging_info_to_file(self, temp_log_file):
        """Test that INFO messages are logged to file."""
        setup_logging(log_file=temp_log_file, log_level="INFO")

        logger = get_logger("test_module")
        logger.info("Test info message")

        with open(temp_log_file) as f:
            content = f.read()

        assert "Test info message" in content
        assert "INFO" in content
        assert "test_module" in content

    def test_logging_warning_to_file(self, temp_log_file):
        """Test that WARNING messages are logged to file."""
        setup_logging(log_file=temp_log_file, log_level="INFO")

        logger = get_logger("test_module")
        logger.warning("Test warning message")

        with open(temp_log_file) as f:
            content = f.read()

        assert "Test warning message" in content
        assert "WARNING" in content

    def test_logging_error_to_file(self, temp_log_file):
        """Test that ERROR messages are logged to file."""
        setup_logging(log_file=temp_log_file, log_level="INFO")

        logger = get_logger("test_module")
        logger.error("Test error message")

        with open(temp_log_file) as f:
            content = f.read()

        assert "Test error message" in content
        assert "ERROR" in content

    def test_logging_debug_filtered_by_level(self, temp_log_file):
        """Test that DEBUG messages are filtered when log_level=INFO."""
        setup_logging(log_file=temp_log_file, log_level="INFO")

        logger = get_logger("test_module")
        logger.debug("Test debug message")

        with open(temp_log_file) as f:
            content = f.read()

        assert "Test debug message" not in content

    def test_logging_debug_included_when_enabled(self, temp_log_file):
        """Test that DEBUG messages are logged when log_level=DEBUG."""
        setup_logging(log_file=temp_log_file, log_level="DEBUG")

        logger = get_logger("test_module")
        logger.debug("Test debug message")

        with open(temp_log_file) as f:
            content = f.read()

        assert "Test debug message" in content
        assert "DEBUG" in content

    def test_logging_with_exception_info(self, temp_log_file):
        """Test that exception info is logged when exc_info=True."""
        setup_logging(log_file=temp_log_file, log_level="ERROR")

        logger = get_logger("test_module")

        try:
            raise ValueError("Test exception")
        except ValueError:
            logger.error("Error occurred", exc_info=True)

        with open(temp_log_file) as f:
            content = f.read()

        assert "Error occurred" in content
        assert "ValueError: Test exception" in content
        assert "Traceback" in content


class TestResetLogging:
    """Tests for reset_logging() function."""

    def test_reset_logging_clears_handlers(self, temp_log_file):
        """Test that reset_logging clears handlers."""
        setup_logging(log_file=temp_log_file)
        assert len(logging.getLogger().handlers) == 2

        reset_logging()

        assert len(logging.getLogger().handlers) == 0

    def test_reset_logging_allows_reinitialization(self, temp_log_file):
        """Test that reset_logging allows setup_logging to run again."""
        setup_logging(log_file=temp_log_file)
        reset_logging()
        setup_logging(log_file=temp_log_file)

        root_logger = logging.getLogger()
        assert len(root_logger.handlers) == 2


class TestMigrationHelpers:
    """Tests for migration helper functions."""

    def test_log_print_replacement_logs_message(self, temp_log_file):
        """Test that log_print_replacement logs message."""
        setup_logging(log_file=temp_log_file, log_level="INFO")

        log_print_replacement("Test message", level="INFO")

        # Check file logging
        with open(temp_log_file) as f:
            content = f.read()
        assert "Test message" in content

    def test_log_print_replacement_warning_level(self, temp_log_file):
        """Test that log_print_replacement handles WARNING level."""
        setup_logging(log_file=temp_log_file, log_level="INFO")

        log_print_replacement("Warning message", level="WARNING")

        with open(temp_log_file) as f:
            content = f.read()
        assert "Warning message" in content
        assert "WARNING" in content

    def test_log_print_replacement_custom_logger(self, temp_log_file):
        """Test that log_print_replacement uses custom logger name."""
        setup_logging(log_file=temp_log_file, log_level="INFO")

        log_print_replacement("Test", level="INFO", logger_name="custom_logger")

        with open(temp_log_file) as f:
            content = f.read()
        assert "custom_logger" in content
