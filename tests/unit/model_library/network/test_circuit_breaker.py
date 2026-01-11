"""Tests for circuit breaker state machine."""

from __future__ import annotations

import time
from unittest import mock

import pytest

from backend.model_library.network.circuit_breaker import CircuitBreaker, CircuitState


@pytest.mark.unit
class TestCircuitState:
    """Tests for CircuitState enum."""

    def test_state_values(self):
        """Test that circuit states have expected values."""
        assert CircuitState.CLOSED.value == "closed"
        assert CircuitState.OPEN.value == "open"
        assert CircuitState.HALF_OPEN.value == "half_open"

    def test_state_from_string(self):
        """Test creating state from string."""
        assert CircuitState("closed") == CircuitState.CLOSED
        assert CircuitState("open") == CircuitState.OPEN
        assert CircuitState("half_open") == CircuitState.HALF_OPEN


@pytest.mark.unit
class TestCircuitBreakerInit:
    """Tests for CircuitBreaker initialization."""

    def test_default_params(self):
        """Test default initialization parameters."""
        cb = CircuitBreaker()
        assert cb.failure_threshold == 5
        assert cb.recovery_timeout == 30.0
        assert cb.half_open_max_calls == 1

    def test_custom_params(self):
        """Test custom initialization parameters."""
        cb = CircuitBreaker(
            failure_threshold=3,
            recovery_timeout=60.0,
            half_open_max_calls=2,
        )
        assert cb.failure_threshold == 3
        assert cb.recovery_timeout == 60.0
        assert cb.half_open_max_calls == 2

    def test_initial_state_is_closed(self):
        """Test that circuit starts in closed state."""
        cb = CircuitBreaker()
        assert cb.state == CircuitState.CLOSED


@pytest.mark.unit
class TestCircuitBreakerClosed:
    """Tests for circuit breaker in closed state."""

    def test_allows_calls_when_closed(self):
        """Test that calls are allowed in closed state."""
        cb = CircuitBreaker(failure_threshold=3)
        assert cb.can_execute() is True

    def test_record_success_keeps_closed(self):
        """Test that success keeps circuit closed."""
        cb = CircuitBreaker(failure_threshold=3)
        cb.record_success()
        assert cb.state == CircuitState.CLOSED

    def test_single_failure_keeps_closed(self):
        """Test that single failure doesn't open circuit."""
        cb = CircuitBreaker(failure_threshold=3)
        cb.record_failure()
        assert cb.state == CircuitState.CLOSED

    def test_failures_below_threshold_keep_closed(self):
        """Test that failures below threshold keep circuit closed."""
        cb = CircuitBreaker(failure_threshold=5)
        for _ in range(4):
            cb.record_failure()
        assert cb.state == CircuitState.CLOSED

    def test_success_resets_failure_count(self):
        """Test that success resets failure counter."""
        cb = CircuitBreaker(failure_threshold=5)
        cb.record_failure()
        cb.record_failure()
        cb.record_success()
        # After success, count is reset, so 4 more failures shouldn't trip
        for _ in range(4):
            cb.record_failure()
        assert cb.state == CircuitState.CLOSED


@pytest.mark.unit
class TestCircuitBreakerOpen:
    """Tests for circuit breaker in open state."""

    def test_opens_after_threshold(self):
        """Test that circuit opens after reaching failure threshold."""
        cb = CircuitBreaker(failure_threshold=3)
        for _ in range(3):
            cb.record_failure()
        assert cb.state == CircuitState.OPEN

    def test_denies_calls_when_open(self):
        """Test that calls are denied in open state."""
        cb = CircuitBreaker(failure_threshold=3)
        for _ in range(3):
            cb.record_failure()
        assert cb.can_execute() is False

    def test_records_open_time(self):
        """Test that opening records timestamp."""
        cb = CircuitBreaker(failure_threshold=3)
        before = time.monotonic()
        for _ in range(3):
            cb.record_failure()
        after = time.monotonic()
        assert before <= cb._opened_at <= after


@pytest.mark.unit
class TestCircuitBreakerHalfOpen:
    """Tests for circuit breaker in half-open state."""

    def test_transitions_to_half_open_after_timeout(self):
        """Test that circuit transitions to half-open after recovery timeout."""
        cb = CircuitBreaker(failure_threshold=3, recovery_timeout=0.1)
        for _ in range(3):
            cb.record_failure()
        assert cb.state == CircuitState.OPEN

        # Wait for recovery timeout
        time.sleep(0.15)
        assert cb.can_execute() is True
        assert cb.state == CircuitState.HALF_OPEN

    def test_half_open_allows_limited_calls(self):
        """Test that half-open allows only limited number of calls."""
        cb = CircuitBreaker(
            failure_threshold=3,
            recovery_timeout=0.1,
            half_open_max_calls=2,
        )
        for _ in range(3):
            cb.record_failure()

        time.sleep(0.15)

        # First two calls allowed
        assert cb.can_execute() is True
        assert cb.can_execute() is True
        # Third call denied
        assert cb.can_execute() is False

    def test_success_in_half_open_closes_circuit(self):
        """Test that success in half-open closes circuit."""
        cb = CircuitBreaker(failure_threshold=3, recovery_timeout=0.1)
        for _ in range(3):
            cb.record_failure()

        time.sleep(0.15)
        cb.can_execute()  # Transition to half-open
        cb.record_success()

        assert cb.state == CircuitState.CLOSED

    def test_failure_in_half_open_reopens_circuit(self):
        """Test that failure in half-open reopens circuit."""
        cb = CircuitBreaker(failure_threshold=3, recovery_timeout=0.1)
        for _ in range(3):
            cb.record_failure()

        time.sleep(0.15)
        cb.can_execute()  # Transition to half-open
        cb.record_failure()

        assert cb.state == CircuitState.OPEN


@pytest.mark.unit
class TestCircuitBreakerStats:
    """Tests for circuit breaker statistics."""

    def test_failure_count_tracking(self):
        """Test that failure count is tracked."""
        cb = CircuitBreaker(failure_threshold=10)
        for _ in range(5):
            cb.record_failure()
        assert cb.failure_count == 5

    def test_total_failures_persist_after_reset(self):
        """Test that total failures persist across resets."""
        cb = CircuitBreaker(failure_threshold=10)
        for _ in range(3):
            cb.record_failure()
        cb.record_success()  # Resets current count
        for _ in range(2):
            cb.record_failure()
        assert cb.total_failures == 5

    def test_total_successes_tracking(self):
        """Test that total successes are tracked."""
        cb = CircuitBreaker()
        for _ in range(3):
            cb.record_success()
        assert cb.total_successes == 3

    def test_get_stats(self):
        """Test getting circuit breaker stats."""
        cb = CircuitBreaker(failure_threshold=10)
        cb.record_success()
        cb.record_failure()
        cb.record_failure()

        stats = cb.get_stats()
        assert stats["state"] == "closed"
        assert stats["failure_count"] == 2
        assert stats["total_failures"] == 2
        assert stats["total_successes"] == 1


@pytest.mark.unit
class TestCircuitBreakerReset:
    """Tests for circuit breaker reset functionality."""

    def test_manual_reset(self):
        """Test manually resetting circuit breaker."""
        cb = CircuitBreaker(failure_threshold=3)
        for _ in range(3):
            cb.record_failure()
        assert cb.state == CircuitState.OPEN

        cb.reset()
        assert cb.state == CircuitState.CLOSED
        assert cb.failure_count == 0

    def test_reset_preserves_total_stats(self):
        """Test that reset preserves total statistics."""
        cb = CircuitBreaker(failure_threshold=3)
        cb.record_success()
        for _ in range(3):
            cb.record_failure()

        cb.reset()
        assert cb.total_failures == 3
        assert cb.total_successes == 1


@pytest.mark.unit
class TestCircuitBreakerEdgeCases:
    """Edge case tests for circuit breaker."""

    def test_multiple_reopen_cycles(self):
        """Test circuit going through multiple open/close cycles."""
        cb = CircuitBreaker(failure_threshold=2, recovery_timeout=0.05)

        # First cycle
        cb.record_failure()
        cb.record_failure()
        assert cb.state == CircuitState.OPEN

        time.sleep(0.1)
        cb.can_execute()  # Half-open
        cb.record_success()  # Closes
        assert cb.state == CircuitState.CLOSED

        # Second cycle
        cb.record_failure()
        cb.record_failure()
        assert cb.state == CircuitState.OPEN

    def test_zero_recovery_timeout(self):
        """Test with zero recovery timeout (immediate half-open)."""
        cb = CircuitBreaker(failure_threshold=2, recovery_timeout=0)
        cb.record_failure()
        cb.record_failure()
        assert cb.state == CircuitState.OPEN

        # Should immediately transition to half-open
        assert cb.can_execute() is True
        assert cb.state == CircuitState.HALF_OPEN

    def test_high_failure_threshold(self):
        """Test with high failure threshold."""
        cb = CircuitBreaker(failure_threshold=100)
        for _ in range(99):
            cb.record_failure()
        assert cb.state == CircuitState.CLOSED

        cb.record_failure()
        assert cb.state == CircuitState.OPEN

    def test_time_in_state(self):
        """Test time spent in current state."""
        cb = CircuitBreaker()
        time.sleep(0.05)
        assert cb.time_in_state() >= 0.05

    def test_consecutive_successes_after_half_open(self):
        """Test consecutive successes after half-open transition."""
        cb = CircuitBreaker(failure_threshold=2, recovery_timeout=0.05)
        cb.record_failure()
        cb.record_failure()

        time.sleep(0.1)
        cb.can_execute()  # Transition to half-open
        cb.record_success()
        assert cb.state == CircuitState.CLOSED

        # Should be able to handle many successes
        for _ in range(10):
            cb.record_success()
        assert cb.state == CircuitState.CLOSED
