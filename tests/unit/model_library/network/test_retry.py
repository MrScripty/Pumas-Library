"""Tests for retry logic with exponential backoff."""

from __future__ import annotations

import asyncio
import time
from typing import Callable
from unittest import mock

import pytest

from backend.model_library.network.retry import (
    RetryConfig,
    RetryError,
    RetryStats,
    calculate_backoff,
    retry,
    retry_async,
    should_retry,
)


@pytest.mark.unit
class TestRetryConfig:
    """Tests for RetryConfig dataclass."""

    def test_default_values(self):
        """Test default configuration values."""
        config = RetryConfig()
        assert config.max_attempts == 3
        assert config.base_delay == 1.0
        assert config.max_delay == 60.0
        assert config.exponential_base == 2.0
        assert config.jitter is True

    def test_custom_values(self):
        """Test custom configuration values."""
        config = RetryConfig(
            max_attempts=5,
            base_delay=0.5,
            max_delay=30.0,
            exponential_base=3.0,
            jitter=False,
        )
        assert config.max_attempts == 5
        assert config.base_delay == 0.5
        assert config.max_delay == 30.0
        assert config.exponential_base == 3.0
        assert config.jitter is False


@pytest.mark.unit
class TestCalculateBackoff:
    """Tests for calculate_backoff function."""

    def test_first_attempt(self):
        """Test backoff for first attempt."""
        config = RetryConfig(base_delay=1.0, exponential_base=2.0, jitter=False)
        delay = calculate_backoff(0, config)
        assert delay == 1.0  # base_delay * 2^0

    def test_second_attempt(self):
        """Test backoff for second attempt."""
        config = RetryConfig(base_delay=1.0, exponential_base=2.0, jitter=False)
        delay = calculate_backoff(1, config)
        assert delay == 2.0  # base_delay * 2^1

    def test_third_attempt(self):
        """Test backoff for third attempt."""
        config = RetryConfig(base_delay=1.0, exponential_base=2.0, jitter=False)
        delay = calculate_backoff(2, config)
        assert delay == 4.0  # base_delay * 2^2

    def test_max_delay_cap(self):
        """Test that delay is capped at max_delay."""
        config = RetryConfig(base_delay=1.0, max_delay=5.0, jitter=False)
        delay = calculate_backoff(10, config)  # Would be 1024 without cap
        assert delay == 5.0

    def test_jitter_within_bounds(self):
        """Test that jitter stays within expected bounds."""
        config = RetryConfig(base_delay=1.0, exponential_base=2.0, jitter=True)
        delays = [calculate_backoff(1, config) for _ in range(100)]
        # With jitter, delay should be between 0 and 2*base_delay for attempt 1
        for delay in delays:
            assert 0 < delay <= 4.0  # 2.0 * 2 for full jitter range

    def test_different_exponential_base(self):
        """Test with different exponential base."""
        config = RetryConfig(base_delay=1.0, exponential_base=3.0, jitter=False)
        assert calculate_backoff(0, config) == 1.0  # 1.0 * 3^0
        assert calculate_backoff(1, config) == 3.0  # 1.0 * 3^1
        assert calculate_backoff(2, config) == 9.0  # 1.0 * 3^2


@pytest.mark.unit
class TestShouldRetry:
    """Tests for should_retry function."""

    def test_retry_on_matching_exception(self):
        """Test that matching exception triggers retry."""
        exc = ValueError("test")
        assert should_retry(exc, [ValueError]) is True

    def test_no_retry_on_non_matching_exception(self):
        """Test that non-matching exception doesn't trigger retry."""
        exc = ValueError("test")
        assert should_retry(exc, [TypeError]) is False

    def test_retry_on_exception_subclass(self):
        """Test that exception subclass triggers retry."""
        exc = FileNotFoundError("test")
        assert should_retry(exc, [OSError]) is True  # FileNotFoundError is OSError

    def test_empty_retryable_list(self):
        """Test with empty retryable exceptions list."""
        exc = ValueError("test")
        assert should_retry(exc, []) is False

    def test_multiple_retryable_exceptions(self):
        """Test with multiple retryable exceptions."""
        exc = TypeError("test")
        assert should_retry(exc, [ValueError, TypeError, OSError]) is True

    def test_custom_predicate(self):
        """Test with custom predicate function."""

        def custom_predicate(exc: BaseException) -> bool:
            return "retry" in str(exc).lower()

        exc1 = ValueError("please retry this")
        exc2 = ValueError("fatal error")

        assert should_retry(exc1, predicate=custom_predicate) is True
        assert should_retry(exc2, predicate=custom_predicate) is False

    def test_predicate_takes_precedence(self):
        """Test that predicate takes precedence over exception types."""

        def always_false(exc: BaseException) -> bool:
            return False

        exc = ValueError("test")
        assert should_retry(exc, [ValueError], predicate=always_false) is False


@pytest.mark.unit
class TestRetryStats:
    """Tests for RetryStats dataclass."""

    def test_default_stats(self):
        """Test default statistics values."""
        stats = RetryStats()
        assert stats.attempts == 0
        assert stats.total_delay == 0.0
        assert stats.success is False
        assert stats.last_exception is None

    def test_record_attempt(self):
        """Test recording an attempt."""
        stats = RetryStats()
        stats.record_attempt(delay=1.5)
        assert stats.attempts == 1
        assert stats.total_delay == 1.5

    def test_record_multiple_attempts(self):
        """Test recording multiple attempts."""
        stats = RetryStats()
        stats.record_attempt(delay=1.0)
        stats.record_attempt(delay=2.0)
        stats.record_attempt(delay=4.0)
        assert stats.attempts == 3
        assert stats.total_delay == 7.0

    def test_record_success(self):
        """Test recording success."""
        stats = RetryStats()
        stats.record_success()
        assert stats.success is True

    def test_record_failure(self):
        """Test recording failure with exception."""
        stats = RetryStats()
        exc = ValueError("test error")
        stats.record_failure(exc)
        assert stats.success is False
        assert stats.last_exception is exc


@pytest.mark.unit
class TestRetrySyncFunction:
    """Tests for synchronous retry function."""

    def test_success_on_first_attempt(self):
        """Test function succeeds on first attempt."""
        mock_fn = mock.Mock(return_value="success")
        config = RetryConfig(max_attempts=3)

        result, stats = retry(mock_fn, config)

        assert result == "success"
        assert stats.attempts == 1
        assert stats.success is True
        mock_fn.assert_called_once()

    def test_success_after_retries(self):
        """Test function succeeds after retries."""
        mock_fn = mock.Mock(side_effect=[ValueError(), ValueError(), "success"])
        config = RetryConfig(max_attempts=3, base_delay=0.01, jitter=False)

        result, stats = retry(mock_fn, config, retryable_exceptions=[ValueError])

        assert result == "success"
        assert stats.attempts == 3
        assert stats.success is True

    def test_max_attempts_exceeded(self):
        """Test raising after max attempts."""
        mock_fn = mock.Mock(side_effect=ValueError("always fails"))
        config = RetryConfig(max_attempts=3, base_delay=0.01, jitter=False)

        with pytest.raises(RetryError) as exc_info:
            retry(mock_fn, config, retryable_exceptions=[ValueError])

        assert exc_info.value.stats.attempts == 3
        assert exc_info.value.stats.success is False
        assert isinstance(exc_info.value.stats.last_exception, ValueError)

    def test_non_retryable_exception_raises_immediately(self):
        """Test that non-retryable exception raises immediately."""
        mock_fn = mock.Mock(side_effect=TypeError("not retryable"))
        config = RetryConfig(max_attempts=3, base_delay=0.01)

        with pytest.raises(TypeError):
            retry(mock_fn, config, retryable_exceptions=[ValueError])

        mock_fn.assert_called_once()

    def test_passes_args_to_function(self):
        """Test that args are passed to function."""
        mock_fn = mock.Mock(return_value="success")
        config = RetryConfig()

        result, _ = retry(mock_fn, config, "arg1", "arg2", kwarg1="value1")

        mock_fn.assert_called_with("arg1", "arg2", kwarg1="value1")

    def test_callback_on_retry(self):
        """Test callback is called on each retry."""
        mock_fn = mock.Mock(side_effect=[ValueError(), ValueError(), "success"])
        callback = mock.Mock()
        config = RetryConfig(max_attempts=3, base_delay=0.01, jitter=False)

        retry(
            mock_fn,
            config,
            retryable_exceptions=[ValueError],
            on_retry=callback,
        )

        assert callback.call_count == 2  # Called before 2nd and 3rd attempts


@pytest.mark.unit
class TestRetryAsyncFunction:
    """Tests for asynchronous retry function."""

    def test_async_success_on_first_attempt(self):
        """Test async function succeeds on first attempt."""

        async def async_fn():
            return "success"

        config = RetryConfig(max_attempts=3)

        async def run_test():
            return await retry_async(async_fn, config)

        result, stats = asyncio.run(run_test())

        assert result == "success"
        assert stats.attempts == 1
        assert stats.success is True

    def test_async_success_after_retries(self):
        """Test async function succeeds after retries."""
        attempt = 0

        async def async_fn():
            nonlocal attempt
            attempt += 1
            if attempt < 3:
                raise ValueError("retry")
            return "success"

        config = RetryConfig(max_attempts=3, base_delay=0.01, jitter=False)

        async def run_test():
            return await retry_async(async_fn, config, retryable_exceptions=[ValueError])

        result, stats = asyncio.run(run_test())

        assert result == "success"
        assert stats.attempts == 3
        assert stats.success is True

    def test_async_max_attempts_exceeded(self):
        """Test async raising after max attempts."""

        async def async_fn():
            raise ValueError("always fails")

        config = RetryConfig(max_attempts=3, base_delay=0.01, jitter=False)

        async def run_test():
            return await retry_async(async_fn, config, retryable_exceptions=[ValueError])

        with pytest.raises(RetryError) as exc_info:
            asyncio.run(run_test())

        assert exc_info.value.stats.attempts == 3
        assert exc_info.value.stats.success is False

    def test_async_passes_args(self):
        """Test async function receives args."""
        received_args: list = []
        received_kwargs: dict = {}

        async def async_fn(*args, **kwargs):
            received_args.extend(args)
            received_kwargs.update(kwargs)
            return "success"

        config = RetryConfig()

        async def run_test():
            return await retry_async(async_fn, config, "a", "b", key="value")

        asyncio.run(run_test())

        assert received_args == ["a", "b"]
        assert received_kwargs == {"key": "value"}


@pytest.mark.unit
class TestRetryEdgeCases:
    """Edge case tests for retry functionality."""

    def test_zero_base_delay(self):
        """Test with zero base delay."""
        config = RetryConfig(base_delay=0.0, jitter=False)
        delay = calculate_backoff(5, config)
        assert delay == 0.0

    def test_single_attempt(self):
        """Test with single attempt allowed."""
        mock_fn = mock.Mock(side_effect=ValueError("fail"))
        config = RetryConfig(max_attempts=1, base_delay=0.01)

        with pytest.raises(RetryError) as exc_info:
            retry(mock_fn, config, retryable_exceptions=[ValueError])

        assert exc_info.value.stats.attempts == 1

    def test_retry_error_contains_original_exception(self):
        """Test RetryError contains original exception."""
        original_exc = ValueError("original error message")
        mock_fn = mock.Mock(side_effect=original_exc)
        config = RetryConfig(max_attempts=2, base_delay=0.01)

        with pytest.raises(RetryError) as exc_info:
            retry(mock_fn, config, retryable_exceptions=[ValueError])

        assert exc_info.value.stats.last_exception is original_exc
        assert "original error message" in str(exc_info.value)

    def test_exception_matching_with_inheritance(self):
        """Test exception matching respects inheritance."""
        mock_fn = mock.Mock(side_effect=[FileNotFoundError(), PermissionError(), "success"])
        config = RetryConfig(max_attempts=3, base_delay=0.01, jitter=False)

        # Both are subclasses of OSError
        result, stats = retry(mock_fn, config, retryable_exceptions=[OSError])

        assert result == "success"
        assert stats.attempts == 3
