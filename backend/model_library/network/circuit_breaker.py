"""Circuit breaker pattern implementation for network resilience.

Provides a circuit breaker state machine that prevents cascading failures
by failing fast when a service is known to be unavailable.

States:
- CLOSED: Normal operation, requests are allowed
- OPEN: Service is failing, requests are rejected immediately
- HALF_OPEN: Testing if service has recovered, limited requests allowed
"""

from __future__ import annotations

import time
from dataclasses import dataclass, field
from enum import Enum
from typing import Any

from backend.logging_config import get_logger

logger = get_logger(__name__)


class CircuitState(Enum):
    """Circuit breaker states."""

    CLOSED = "closed"
    OPEN = "open"
    HALF_OPEN = "half_open"


@dataclass
class CircuitBreaker:
    """Circuit breaker for fail-fast behavior on failing services.

    The circuit breaker tracks failures and opens (trips) after reaching
    a threshold. While open, it rejects requests immediately. After a
    recovery timeout, it transitions to half-open to test if the service
    has recovered.

    Attributes:
        failure_threshold: Number of failures before circuit opens
        recovery_timeout: Seconds to wait before attempting recovery
        half_open_max_calls: Max calls allowed in half-open state
    """

    failure_threshold: int = 5
    recovery_timeout: float = 30.0
    half_open_max_calls: int = 1

    # Internal state (not part of init)
    _state: CircuitState = field(default=CircuitState.CLOSED, init=False)
    _failure_count: int = field(default=0, init=False)
    _total_failures: int = field(default=0, init=False)
    _total_successes: int = field(default=0, init=False)
    _opened_at: float = field(default=0.0, init=False)
    _state_entered_at: float = field(default=0.0, init=False)
    _half_open_calls: int = field(default=0, init=False)

    def __post_init__(self) -> None:
        """Initialize timestamps after dataclass init."""
        self._state_entered_at = time.monotonic()

    @property
    def state(self) -> CircuitState:
        """Get current circuit state."""
        return self._state

    @property
    def failure_count(self) -> int:
        """Get current failure count (resets on success)."""
        return self._failure_count

    @property
    def total_failures(self) -> int:
        """Get total failures since creation."""
        return self._total_failures

    @property
    def total_successes(self) -> int:
        """Get total successes since creation."""
        return self._total_successes

    def _set_state(self, new_state: CircuitState) -> None:
        """Transition to a new state.

        Args:
            new_state: The state to transition to
        """
        if new_state != self._state:
            old_state = self._state
            self._state = new_state
            self._state_entered_at = time.monotonic()

            if new_state == CircuitState.OPEN:
                self._opened_at = self._state_entered_at
            elif new_state == CircuitState.HALF_OPEN:
                self._half_open_calls = 0

            logger.debug(
                "Circuit breaker state change: %s -> %s",
                old_state.value,
                new_state.value,
            )

    def can_execute(self) -> bool:
        """Check if a call can be executed.

        Returns:
            True if call should proceed, False if it should be rejected
        """
        if self._state == CircuitState.CLOSED:
            return True

        if self._state == CircuitState.OPEN:
            # Check if recovery timeout has elapsed
            elapsed = time.monotonic() - self._opened_at
            if elapsed >= self.recovery_timeout:
                self._set_state(CircuitState.HALF_OPEN)
                self._half_open_calls += 1
                return True
            return False

        # HALF_OPEN state
        if self._half_open_calls < self.half_open_max_calls:
            self._half_open_calls += 1
            return True
        return False

    def record_success(self) -> None:
        """Record a successful call.

        In CLOSED state: resets failure count
        In HALF_OPEN state: closes circuit (service recovered)
        In OPEN state: ignored
        """
        self._total_successes += 1

        if self._state == CircuitState.CLOSED:
            self._failure_count = 0

        elif self._state == CircuitState.HALF_OPEN:
            logger.info("Circuit breaker recovered, closing circuit")
            self._set_state(CircuitState.CLOSED)
            self._failure_count = 0

    def record_failure(self) -> None:
        """Record a failed call.

        In CLOSED state: increments failure count, may open circuit
        In HALF_OPEN state: reopens circuit
        In OPEN state: ignored
        """
        self._total_failures += 1
        self._failure_count += 1

        if self._state == CircuitState.CLOSED:
            if self._failure_count >= self.failure_threshold:
                logger.warning(
                    "Circuit breaker tripped after %d failures",
                    self._failure_count,
                )
                self._set_state(CircuitState.OPEN)

        elif self._state == CircuitState.HALF_OPEN:
            logger.warning("Circuit breaker failed in half-open state, reopening")
            self._set_state(CircuitState.OPEN)

    def reset(self) -> None:
        """Manually reset the circuit breaker to closed state.

        Preserves total statistics but resets current failure count.
        """
        logger.info("Circuit breaker manually reset")
        self._set_state(CircuitState.CLOSED)
        self._failure_count = 0
        self._half_open_calls = 0

    def time_in_state(self) -> float:
        """Get time spent in current state.

        Returns:
            Seconds spent in current state
        """
        return time.monotonic() - self._state_entered_at

    def get_stats(self) -> dict[str, Any]:
        """Get circuit breaker statistics.

        Returns:
            Dictionary containing current stats
        """
        return {
            "state": self._state.value,
            "failure_count": self._failure_count,
            "total_failures": self._total_failures,
            "total_successes": self._total_successes,
            "time_in_state": self.time_in_state(),
        }
