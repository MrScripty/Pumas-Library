"""
Centralized logging configuration for ComfyUI Launcher.

This module provides a structured logging system to replace the 456 print() statements
throughout the codebase. It offers:
- Rotating file logs (10MB max, 5 backups) for detailed troubleshooting
- Console output for user-facing messages (WARNING+ levels)
- Per-module loggers with consistent formatting
- Easy integration with existing code

Usage:
    from backend.logging_config import get_logger

    logger = get_logger(__name__)
    logger.info("Operation completed")
    logger.warning("Resource usage high")
    logger.error("Failed to connect", exc_info=True)
"""

import logging
import sys
from logging.handlers import RotatingFileHandler
from pathlib import Path
from typing import Optional

# Global flag to track if logging is initialized
_logging_initialized = False


def setup_logging(
    log_level: str = "INFO",
    log_file: Optional[Path] = None,
    max_bytes: int = 10_485_760,  # 10MB
    backup_count: int = 5,
    console_level: str = "WARNING",
) -> None:
    """
    Configure the root logger with file and console handlers.

    This should be called once at application startup. Subsequent calls will be ignored
    to prevent duplicate handlers.

    Args:
        log_level: Minimum level for file logging (DEBUG, INFO, WARNING, ERROR, CRITICAL)
        log_file: Path to log file. If None, uses launcher-data/logs/launcher.log
        max_bytes: Maximum size of log file before rotation (default: 10MB)
        backup_count: Number of backup log files to keep (default: 5)
        console_level: Minimum level for console output (default: WARNING)
    """
    global _logging_initialized

    if _logging_initialized:
        return

    # Get root logger
    root_logger = logging.getLogger()
    root_logger.setLevel(logging.DEBUG)  # Capture everything, filter in handlers

    # Clear any existing handlers (defensive)
    root_logger.handlers.clear()

    # Create formatters
    detailed_formatter = logging.Formatter(
        fmt="%(asctime)s - %(name)s - %(levelname)s - %(message)s",
        datefmt="%Y-%m-%d %H:%M:%S",
    )

    console_formatter = logging.Formatter(
        fmt="%(levelname)s: %(message)s",  # Simpler format for console
    )

    # File handler - detailed logs for troubleshooting
    if log_file is None:
        # Default log location: launcher-data/logs/launcher.log
        # Note: This assumes the current working directory is the launcher root
        launcher_root = Path.cwd()
        log_dir = launcher_root / "launcher-data" / "logs"
        log_dir.mkdir(parents=True, exist_ok=True)
        log_file = log_dir / "launcher.log"
    else:
        # Ensure parent directory exists for custom log file paths
        log_file = Path(log_file)
        log_file.parent.mkdir(parents=True, exist_ok=True)

    file_handler = RotatingFileHandler(
        filename=log_file,
        maxBytes=max_bytes,
        backupCount=backup_count,
        encoding="utf-8",
    )
    file_handler.setLevel(getattr(logging, log_level.upper()))
    file_handler.setFormatter(detailed_formatter)
    root_logger.addHandler(file_handler)

    # Console handler - only warnings and errors to avoid spam
    console_handler = logging.StreamHandler(sys.stdout)
    console_handler.setLevel(getattr(logging, console_level.upper()))
    console_handler.setFormatter(console_formatter)
    root_logger.addHandler(console_handler)

    _logging_initialized = True

    # Log initialization message
    root_logger.info(
        f"Logging initialized: file={log_file} (level={log_level}), "
        f"console (level={console_level})"
    )


def get_logger(name: str) -> logging.Logger:
    """
    Get a logger instance for a specific module.

    This is the primary way to obtain loggers throughout the application.
    Call setup_logging() once at startup before using this function.

    Args:
        name: Logger name, typically __name__ of the calling module

    Returns:
        Logger instance configured with the application's settings

    Example:
        logger = get_logger(__name__)
        logger.info("Processing started")
        logger.error("Failed to process", exc_info=True)
    """
    # Ensure logging is initialized
    if not _logging_initialized:
        setup_logging()

    return logging.getLogger(name)


def reset_logging() -> None:
    """
    Reset logging configuration (primarily for testing).

    This clears all handlers and resets the initialization flag,
    allowing setup_logging() to be called again.
    """
    global _logging_initialized

    root_logger = logging.getLogger()

    # Properly close all handlers to avoid resource warnings
    for handler in root_logger.handlers[:]:
        handler.close()
        root_logger.removeHandler(handler)

    root_logger.setLevel(logging.WARNING)
    _logging_initialized = False


# Migration helpers for gradual adoption
def log_print_replacement(message: str, level: str = "INFO", logger_name: str = "backend") -> None:
    """
    Temporary helper for replacing print() calls during migration.

    This function logs a message and also prints it to stdout, allowing
    gradual migration from print() to proper logging. Once migration is
    complete, this function can be removed.

    Args:
        message: Message to log and print
        level: Log level (INFO, WARNING, ERROR, etc.)
        logger_name: Name of logger to use

    Example:
        # Before:
        print(f"Installing version {tag}")

        # During migration:
        log_print_replacement(f"Installing version {tag}", "INFO")

        # After migration:
        logger.info(f"Installing version {tag}")
    """
    logger = get_logger(logger_name)
    log_method = getattr(logger, level.lower(), logger.info)
    log_method(message)
    print(message)  # Also print for backwards compatibility
