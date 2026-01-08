#!/usr/bin/env python3
"""
Retry utilities with exponential backoff and jitter

Provides utilities for retrying operations with exponential backoff to
prevent thundering herd problems and improve reliability.
"""

import random
import time
from typing import Callable, Optional, TypeVar

from backend.logging_config import get_logger

logger = get_logger(__name__)

T = TypeVar("T")


def calculate_backoff_delay(
    attempt: int, base_delay: float = 2.0, max_delay: float = 60.0, jitter: bool = True
) -> float:
    """
    Calculate exponential backoff delay with optional jitter

    Args:
        attempt: Current attempt number (0-indexed, so attempt 0 = first retry)
        base_delay: Base delay in seconds (default: 2.0)
        max_delay: Maximum delay in seconds (default: 60.0)
        jitter: Whether to add random jitter (default: True)

    Returns:
        Delay in seconds

    Example:
        >>> calculate_backoff_delay(0)  # First retry
        ~2.0-3.0 seconds (2^1 + jitter)
        >>> calculate_backoff_delay(1)  # Second retry
        ~4.0-5.0 seconds (2^2 + jitter)
        >>> calculate_backoff_delay(2)  # Third retry
        ~8.0-9.0 seconds (2^3 + jitter)
    """
    # Exponential backoff: 2^(attempt + 1) seconds
    # attempt 0 (first retry) = 2^1 = 2s
    # attempt 1 (second retry) = 2^2 = 4s
    # attempt 2 (third retry) = 2^3 = 8s
    delay = base_delay ** (attempt + 1)

    # Cap at max_delay
    delay = min(delay, max_delay)

    # Add jitter: random 0-1 seconds
    if jitter:
        delay += random.uniform(0, 1)

    return delay


def retry_with_backoff(
    func: Callable[[], T],
    max_retries: int = 3,
    base_delay: float = 2.0,
    max_delay: float = 60.0,
    on_retry: Optional[Callable[[int, float, Exception], None]] = None,
    exceptions: tuple[type[Exception], ...] | type[Exception] = Exception,
    exception_type: Optional[type[Exception]] = None,
) -> Optional[T]:
    """
    Retry a function with exponential backoff and jitter

    Args:
        func: Function to retry (should raise exception on failure)
        max_retries: Maximum number of retry attempts
        base_delay: Base delay in seconds for exponential backoff
        max_delay: Maximum delay in seconds
        on_retry: Optional callback function(attempt, delay, error) called before each retry
        exceptions: Exception types to catch and retry
        exception_type: Deprecated alias for exceptions (kept for backward compatibility)

    Returns:
        Result of func if successful, None if all retries failed

    Example:
        >>> def fetch_data():
        ...     response = urllib.request.urlopen("https://api.example.com/data")
        ...     return response.read()
        >>> result = retry_with_backoff(fetch_data, max_retries=3, exceptions=(OSError,))
    """
    if exception_type is not None:
        exceptions = exception_type

    exceptions_to_retry: tuple[type[Exception], ...]
    if isinstance(exceptions, type):
        exceptions_to_retry = (exceptions,)
    else:
        exceptions_to_retry = tuple(exceptions)

    last_exception = None

    for attempt in range(max_retries + 1):  # +1 for initial attempt
        try:
            return func()
        except exceptions_to_retry as e:
            last_exception = e
            logger.debug("Retryable error: %s", e)

            # If this was the last attempt, don't retry
            if attempt >= max_retries:
                break

            # Calculate backoff delay
            delay = calculate_backoff_delay(attempt, base_delay, max_delay)

            # Call retry callback if provided
            if on_retry:
                on_retry(attempt, delay, e)

            # Wait before retrying
            time.sleep(delay)

    # All retries failed
    return None


def retry_operation(
    operation: Callable[[], bool],
    max_retries: int = 3,
    base_delay: float = 2.0,
    operation_name: str = "Operation",
) -> bool:
    """
    Retry a boolean operation with exponential backoff

    Simplified retry function for operations that return True/False.
    Automatically prints retry messages.

    Args:
        operation: Function that returns True on success, False on failure
        max_retries: Maximum number of retry attempts
        base_delay: Base delay in seconds for exponential backoff
        operation_name: Name of operation for logging

    Returns:
        True if successful, False if all retries failed

    Example:
        >>> def download():
        ...     return download_file(url, destination)
        >>> success = retry_operation(download, max_retries=3, operation_name="Download")
    """

    def on_retry_callback(attempt: int, delay: float, error: Exception) -> None:
        """Log retry message"""
        logger.info(
            f"{operation_name} failed (attempt {attempt + 1}/{max_retries}). "
            f"Retrying in {delay:.1f}s... ({type(error).__name__}: {error})"
        )

    # Wrapper to convert boolean return to exception-based
    def operation_wrapper() -> bool:
        result = operation()
        if not result:
            raise RuntimeError(f"{operation_name} returned False")
        return result

    try:
        result = retry_with_backoff(
            operation_wrapper,
            max_retries=max_retries,
            base_delay=base_delay,
            on_retry=on_retry_callback,
        )
        return result is not None
    except RuntimeError as e:
        logger.info(f"{operation_name} failed after {max_retries} attempts: {e}")
        return False
