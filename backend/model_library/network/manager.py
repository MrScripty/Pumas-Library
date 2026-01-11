"""NetworkManager coordinator for network operations.

Provides a unified interface for network requests with integrated
circuit breaker and retry functionality.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any, Callable
from urllib.parse import urlparse

from backend.logging_config import get_logger
from backend.model_library.network.circuit_breaker import CircuitBreaker, CircuitState
from backend.model_library.network.retry import RetryConfig, RetryError, retry, retry_async

logger = get_logger(__name__)


class NetworkError(Exception):
    """Raised when a network operation fails."""

    pass


def extract_domain(url: str) -> str:
    """Extract domain from URL.

    Args:
        url: URL string

    Returns:
        Domain name or empty string if invalid
    """
    if not url:
        return ""

    # Handle URLs without scheme
    if "://" not in url:
        url = "https://" + url

    try:
        parsed = urlparse(url)
        return parsed.hostname or parsed.netloc.split(":")[0] or ""
    except ValueError:  # noqa: no-except-logging
        return ""


@dataclass
class NetworkStats:
    """Statistics for network operations.

    Attributes:
        total_requests: Total number of requests made
        successful_requests: Number of successful requests
        failed_requests: Number of failed requests
        circuit_breaker_rejections: Requests rejected by circuit breaker
        retries: Total number of retry attempts
    """

    total_requests: int = 0
    successful_requests: int = 0
    failed_requests: int = 0
    circuit_breaker_rejections: int = 0
    retries: int = 0

    @property
    def success_rate(self) -> float:
        """Calculate success rate as percentage."""
        if self.total_requests == 0:
            return 0.0
        return (self.successful_requests / self.total_requests) * 100


@dataclass
class DomainStats:
    """Statistics for a specific domain."""

    successes: int = 0
    failures: int = 0
    last_error: str | None = None


class NetworkManager:
    """Coordinates network operations with circuit breaker and retry logic.

    Provides a unified interface for making network requests with automatic
    retry handling and circuit breaker protection per domain.

    Attributes:
        retry_config: Configuration for retry behavior
        default_timeout: Default timeout for requests in seconds
    """

    def __init__(
        self,
        retry_config: RetryConfig | None = None,
        default_timeout: float = 30.0,
        circuit_failure_threshold: int = 5,
        circuit_recovery_timeout: float = 30.0,
    ) -> None:
        """Initialize NetworkManager.

        Args:
            retry_config: Retry configuration (uses defaults if None)
            default_timeout: Default request timeout in seconds
            circuit_failure_threshold: Failures before circuit opens
            circuit_recovery_timeout: Seconds before circuit half-opens
        """
        self.retry_config = retry_config or RetryConfig()
        self.default_timeout = default_timeout
        self._circuit_failure_threshold = circuit_failure_threshold
        self._circuit_recovery_timeout = circuit_recovery_timeout

        self._circuit_breakers: dict[str, CircuitBreaker] = {}
        self._stats = NetworkStats()
        self._domain_stats: dict[str, DomainStats] = {}

    def get_circuit_breaker(self, domain: str) -> CircuitBreaker:
        """Get or create circuit breaker for domain.

        Args:
            domain: Domain name

        Returns:
            CircuitBreaker instance for the domain
        """
        if domain not in self._circuit_breakers:
            self._circuit_breakers[domain] = CircuitBreaker(
                failure_threshold=self._circuit_failure_threshold,
                recovery_timeout=self._circuit_recovery_timeout,
            )
        return self._circuit_breakers[domain]

    def is_circuit_open(self, domain: str) -> bool:
        """Check if circuit breaker is open for domain.

        Args:
            domain: Domain name

        Returns:
            True if circuit is open (requests should be rejected)
        """
        if domain not in self._circuit_breakers:
            return False
        cb = self._circuit_breakers[domain]
        return cb.state == CircuitState.OPEN

    def reset_circuit(self, domain: str) -> None:
        """Reset circuit breaker for domain.

        Args:
            domain: Domain name
        """
        if domain in self._circuit_breakers:
            self._circuit_breakers[domain].reset()
            logger.info("Reset circuit breaker for %s", domain)

    def get_stats(self) -> NetworkStats:
        """Get overall network statistics.

        Returns:
            NetworkStats instance
        """
        return self._stats

    def get_domain_stats(self) -> dict[str, dict[str, Any]]:
        """Get statistics per domain.

        Returns:
            Dictionary mapping domain to stats dict
        """
        return {
            domain: {
                "successes": stats.successes,
                "failures": stats.failures,
                "last_error": stats.last_error,
            }
            for domain, stats in self._domain_stats.items()
        }

    def get_all_circuit_states(self) -> dict[str, str]:
        """Get state of all circuit breakers.

        Returns:
            Dictionary mapping domain to state string
        """
        return {domain: cb.state.value for domain, cb in self._circuit_breakers.items()}

    def clear_stats(self) -> None:
        """Clear all statistics."""
        self._stats = NetworkStats()
        self._domain_stats.clear()

    def _record_success(self, domain: str) -> None:
        """Record successful request for domain."""
        self._stats.total_requests += 1
        self._stats.successful_requests += 1

        if domain not in self._domain_stats:
            self._domain_stats[domain] = DomainStats()
        self._domain_stats[domain].successes += 1

        # Record success in circuit breaker
        if domain in self._circuit_breakers:
            self._circuit_breakers[domain].record_success()

    def _record_failure(self, domain: str, error: BaseException) -> None:
        """Record failed request for domain."""
        self._stats.total_requests += 1
        self._stats.failed_requests += 1

        if domain not in self._domain_stats:
            self._domain_stats[domain] = DomainStats()
        self._domain_stats[domain].failures += 1
        self._domain_stats[domain].last_error = str(error)

        # Record failure in circuit breaker
        if domain in self._circuit_breakers:
            self._circuit_breakers[domain].record_failure()

    def _record_circuit_rejection(self, domain: str) -> None:
        """Record circuit breaker rejection."""
        self._stats.circuit_breaker_rejections += 1

    def execute(
        self,
        fn: Callable[..., Any],
        url: str,
        *args: Any,
        retryable_exceptions: list[type[BaseException]] | None = None,
        **kwargs: Any,
    ) -> Any:
        """Execute a function with circuit breaker and retry logic.

        Args:
            fn: Function to execute (should make the network request)
            url: URL for circuit breaker domain extraction
            *args: Positional arguments for fn
            retryable_exceptions: Exception types that should trigger retry
            **kwargs: Keyword arguments for fn

        Returns:
            Result from fn

        Raises:
            NetworkError: If circuit is open or all retries exhausted
        """
        domain = extract_domain(url)
        cb = self.get_circuit_breaker(domain)

        # Check circuit breaker
        if not cb.can_execute():
            self._record_circuit_rejection(domain)
            logger.warning("Request rejected by circuit breaker for %s", domain)
            raise NetworkError(f"Circuit breaker open for {domain}")

        try:
            result, stats = retry(
                fn,
                self.retry_config,
                *args,
                retryable_exceptions=retryable_exceptions,
                **kwargs,
            )

            # Record stats
            self._stats.retries += stats.attempts - 1
            self._record_success(domain)

            return result

        except RetryError as e:
            self._stats.retries += e.stats.attempts - 1
            self._record_failure(domain, e)
            logger.error("Request to %s failed after %d attempts", domain, e.stats.attempts)
            raise NetworkError(
                f"Request to {domain} failed after {e.stats.attempts} attempts"
            ) from e

        except BaseException as e:  # noqa: generic-exception
            self._record_failure(domain, e)
            logger.error("Request to %s failed: %s", domain, e)
            raise

    async def execute_async(
        self,
        fn: Callable[..., Any],
        url: str,
        *args: Any,
        retryable_exceptions: list[type[BaseException]] | None = None,
        **kwargs: Any,
    ) -> Any:
        """Execute an async function with circuit breaker and retry logic.

        Args:
            fn: Async function to execute
            url: URL for circuit breaker domain extraction
            *args: Positional arguments for fn
            retryable_exceptions: Exception types that should trigger retry
            **kwargs: Keyword arguments for fn

        Returns:
            Result from fn

        Raises:
            NetworkError: If circuit is open or all retries exhausted
        """
        domain = extract_domain(url)
        cb = self.get_circuit_breaker(domain)

        # Check circuit breaker
        if not cb.can_execute():
            self._record_circuit_rejection(domain)
            logger.warning("Async request rejected by circuit breaker for %s", domain)
            raise NetworkError(f"Circuit breaker open for {domain}")

        try:
            result, stats = await retry_async(
                fn,
                self.retry_config,
                *args,
                retryable_exceptions=retryable_exceptions,
                **kwargs,
            )

            # Record stats
            self._stats.retries += stats.attempts - 1
            self._record_success(domain)

            return result

        except RetryError as e:
            self._stats.retries += e.stats.attempts - 1
            self._record_failure(domain, e)
            logger.error("Async request to %s failed after %d attempts", domain, e.stats.attempts)
            raise NetworkError(
                f"Async request to {domain} failed after {e.stats.attempts} attempts"
            ) from e

        except BaseException as e:  # noqa: generic-exception
            self._record_failure(domain, e)
            logger.error("Async request to %s failed: %s", domain, e)
            raise
