"""Retry logic with exponential backoff for network operations.

Provides configurable retry behavior with exponential backoff and jitter
to handle transient network failures gracefully.
"""

from __future__ import annotations

import asyncio
import random
import time
from dataclasses import dataclass, field
from typing import Any, Callable, TypeVar

from backend.logging_config import get_logger

logger = get_logger(__name__)

T = TypeVar("T")


@dataclass
class RetryConfig:
    """Configuration for retry behavior.

    Attributes:
        max_attempts: Maximum number of attempts (including first try)
        base_delay: Initial delay in seconds
        max_delay: Maximum delay cap in seconds
        exponential_base: Base for exponential calculation
        jitter: Whether to add randomness to delays
    """

    max_attempts: int = 3
    base_delay: float = 1.0
    max_delay: float = 60.0
    exponential_base: float = 2.0
    jitter: bool = True


@dataclass
class RetryStats:
    """Statistics from a retry operation.

    Attributes:
        attempts: Number of attempts made
        total_delay: Total time spent waiting between retries
        success: Whether the operation ultimately succeeded
        last_exception: The last exception raised, if any
    """

    attempts: int = 0
    total_delay: float = 0.0
    success: bool = False
    last_exception: BaseException | None = None

    def record_attempt(self, delay: float = 0.0) -> None:
        """Record an attempt with optional delay."""
        self.attempts += 1
        self.total_delay += delay

    def record_success(self) -> None:
        """Mark operation as successful."""
        self.success = True

    def record_failure(self, exc: BaseException) -> None:
        """Record a failure with exception."""
        self.success = False
        self.last_exception = exc


class RetryError(Exception):
    """Raised when all retry attempts are exhausted.

    Attributes:
        stats: Statistics from the retry operation
    """

    def __init__(self, message: str, stats: RetryStats) -> None:
        super().__init__(message)
        self.stats = stats

    def __str__(self) -> str:
        base_msg = super().__str__()
        if self.stats.last_exception:
            return f"{base_msg}: {self.stats.last_exception}"
        return base_msg


def calculate_backoff(attempt: int, config: RetryConfig) -> float:
    """Calculate backoff delay for a given attempt.

    Uses exponential backoff with optional jitter.

    Args:
        attempt: The current attempt number (0-indexed)
        config: Retry configuration

    Returns:
        Delay in seconds before next retry
    """
    # Exponential backoff: base_delay * exponential_base ^ attempt
    delay = config.base_delay * (config.exponential_base**attempt)

    # Cap at max_delay
    delay = min(delay, config.max_delay)

    # Add jitter if enabled (random value between 0 and delay)
    if config.jitter and delay > 0:
        delay = random.uniform(0, delay * 2)  # noqa: S311
        delay = min(delay, config.max_delay)

    return delay


def should_retry(
    exc: BaseException,
    retryable_exceptions: list[type[BaseException]] | None = None,
    predicate: Callable[[BaseException], bool] | None = None,
) -> bool:
    """Determine if an exception should trigger a retry.

    Args:
        exc: The exception that was raised
        retryable_exceptions: List of exception types that are retryable
        predicate: Custom function to determine if exception is retryable

    Returns:
        True if the operation should be retried
    """
    # Predicate takes precedence
    if predicate is not None:
        return predicate(exc)

    # Check against exception types
    if retryable_exceptions:
        return isinstance(exc, tuple(retryable_exceptions))

    return False


def retry(
    fn: Callable[..., T],
    config: RetryConfig,
    *args: Any,
    retryable_exceptions: list[type[BaseException]] | None = None,
    predicate: Callable[[BaseException], bool] | None = None,
    on_retry: Callable[[int, BaseException, float], None] | None = None,
    **kwargs: Any,
) -> tuple[T, RetryStats]:
    """Execute a function with retry logic.

    Args:
        fn: Function to execute
        config: Retry configuration
        *args: Positional arguments for fn
        retryable_exceptions: Exception types that trigger retry
        predicate: Custom predicate to determine if retry should occur
        on_retry: Callback called before each retry (attempt, exception, delay)
        **kwargs: Keyword arguments for fn

    Returns:
        Tuple of (result, stats)

    Raises:
        RetryError: If all attempts are exhausted
        Exception: Non-retryable exceptions are raised immediately
    """
    stats = RetryStats()

    for attempt in range(config.max_attempts):
        try:
            stats.record_attempt()
            result = fn(*args, **kwargs)
            stats.record_success()
            return result, stats

        except BaseException as exc:  # noqa: generic-exception
            stats.record_failure(exc)

            # Check if we should retry
            if not should_retry(exc, retryable_exceptions, predicate):
                raise

            # Check if we have attempts remaining
            if attempt + 1 >= config.max_attempts:
                logger.warning(
                    "Retry exhausted after %d attempts: %s",
                    stats.attempts,
                    exc,
                )
                raise RetryError(
                    f"All {config.max_attempts} attempts failed",
                    stats,
                ) from exc

            # Calculate and apply backoff
            delay = calculate_backoff(attempt, config)
            stats.total_delay += delay

            logger.debug(
                "Attempt %d failed, retrying in %.2fs: %s",
                attempt + 1,
                delay,
                exc,
            )

            if on_retry:
                on_retry(attempt + 1, exc, delay)

            time.sleep(delay)

    # Should not reach here, but satisfy type checker
    raise RetryError(f"All {config.max_attempts} attempts failed", stats)


async def retry_async(
    fn: Callable[..., Any],
    config: RetryConfig,
    *args: Any,
    retryable_exceptions: list[type[BaseException]] | None = None,
    predicate: Callable[[BaseException], bool] | None = None,
    on_retry: Callable[[int, BaseException, float], None] | None = None,
    **kwargs: Any,
) -> tuple[Any, RetryStats]:
    """Execute an async function with retry logic.

    Args:
        fn: Async function to execute
        config: Retry configuration
        *args: Positional arguments for fn
        retryable_exceptions: Exception types that trigger retry
        predicate: Custom predicate to determine if retry should occur
        on_retry: Callback called before each retry (attempt, exception, delay)
        **kwargs: Keyword arguments for fn

    Returns:
        Tuple of (result, stats)

    Raises:
        RetryError: If all attempts are exhausted
        Exception: Non-retryable exceptions are raised immediately
    """
    stats = RetryStats()

    for attempt in range(config.max_attempts):
        try:
            stats.record_attempt()
            result = await fn(*args, **kwargs)
            stats.record_success()
            return result, stats

        except BaseException as exc:  # noqa: generic-exception
            stats.record_failure(exc)

            # Check if we should retry
            if not should_retry(exc, retryable_exceptions, predicate):
                raise

            # Check if we have attempts remaining
            if attempt + 1 >= config.max_attempts:
                logger.warning(
                    "Async retry exhausted after %d attempts: %s",
                    stats.attempts,
                    exc,
                )
                raise RetryError(
                    f"All {config.max_attempts} attempts failed",
                    stats,
                ) from exc

            # Calculate and apply backoff
            delay = calculate_backoff(attempt, config)
            stats.total_delay += delay

            logger.debug(
                "Async attempt %d failed, retrying in %.2fs: %s",
                attempt + 1,
                delay,
                exc,
            )

            if on_retry:
                on_retry(attempt + 1, exc, delay)

            await asyncio.sleep(delay)

    # Should not reach here, but satisfy type checker
    raise RetryError(f"All {config.max_attempts} attempts failed", stats)
