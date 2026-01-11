"""Tests for NetworkManager coordinator."""

from __future__ import annotations

import asyncio
from pathlib import Path
from typing import Any
from unittest import mock

import pytest

from backend.model_library.network.circuit_breaker import CircuitBreaker, CircuitState
from backend.model_library.network.manager import (
    NetworkError,
    NetworkManager,
    NetworkStats,
    extract_domain,
)
from backend.model_library.network.retry import RetryConfig


@pytest.mark.unit
class TestExtractDomain:
    """Tests for extract_domain function."""

    def test_simple_url(self):
        """Test extracting domain from simple URL."""
        assert extract_domain("https://example.com/path") == "example.com"

    def test_url_with_port(self):
        """Test extracting domain with port."""
        assert extract_domain("https://example.com:8080/path") == "example.com"

    def test_url_with_subdomain(self):
        """Test extracting domain with subdomain."""
        assert extract_domain("https://api.example.com/v1") == "api.example.com"

    def test_url_without_scheme(self):
        """Test URL without scheme."""
        assert extract_domain("example.com/path") == "example.com"

    def test_huggingface_url(self):
        """Test HuggingFace URL."""
        assert extract_domain("https://huggingface.co/models") == "huggingface.co"

    def test_localhost(self):
        """Test localhost URL."""
        assert extract_domain("http://localhost:3000/api") == "localhost"

    def test_invalid_url(self):
        """Test invalid URL returns empty string."""
        assert extract_domain("") == ""
        assert extract_domain("not-a-url") == "not-a-url"


@pytest.mark.unit
class TestNetworkStats:
    """Tests for NetworkStats dataclass."""

    def test_default_values(self):
        """Test default statistics values."""
        stats = NetworkStats()
        assert stats.total_requests == 0
        assert stats.successful_requests == 0
        assert stats.failed_requests == 0
        assert stats.circuit_breaker_rejections == 0
        assert stats.retries == 0

    def test_success_rate_no_requests(self):
        """Test success rate with no requests."""
        stats = NetworkStats()
        assert stats.success_rate == 0.0

    def test_success_rate_all_success(self):
        """Test success rate with all successful requests."""
        stats = NetworkStats(total_requests=10, successful_requests=10)
        assert stats.success_rate == 100.0

    def test_success_rate_mixed(self):
        """Test success rate with mixed results."""
        stats = NetworkStats(total_requests=10, successful_requests=7, failed_requests=3)
        assert stats.success_rate == 70.0


@pytest.mark.unit
class TestNetworkManagerInit:
    """Tests for NetworkManager initialization."""

    def test_default_init(self):
        """Test default initialization."""
        manager = NetworkManager()
        assert manager.retry_config.max_attempts == 3
        assert manager.default_timeout == 30.0

    def test_custom_init(self):
        """Test custom initialization."""
        config = RetryConfig(max_attempts=5, base_delay=0.5)
        manager = NetworkManager(retry_config=config, default_timeout=60.0)
        assert manager.retry_config.max_attempts == 5
        assert manager.default_timeout == 60.0


@pytest.mark.unit
class TestNetworkManagerCircuitBreaker:
    """Tests for NetworkManager circuit breaker functionality."""

    def test_get_circuit_breaker_creates_new(self):
        """Test that get_circuit_breaker creates new breaker for domain."""
        manager = NetworkManager()
        cb = manager.get_circuit_breaker("example.com")
        assert cb is not None
        assert cb.state == CircuitState.CLOSED

    def test_get_circuit_breaker_returns_same(self):
        """Test that same breaker is returned for same domain."""
        manager = NetworkManager()
        cb1 = manager.get_circuit_breaker("example.com")
        cb2 = manager.get_circuit_breaker("example.com")
        assert cb1 is cb2

    def test_get_circuit_breaker_different_domains(self):
        """Test that different domains get different breakers."""
        manager = NetworkManager()
        cb1 = manager.get_circuit_breaker("example1.com")
        cb2 = manager.get_circuit_breaker("example2.com")
        assert cb1 is not cb2

    def test_is_circuit_open_closed(self):
        """Test is_circuit_open returns False for closed circuit."""
        manager = NetworkManager()
        assert manager.is_circuit_open("example.com") is False

    def test_is_circuit_open_after_failures(self):
        """Test is_circuit_open returns True after failures."""
        manager = NetworkManager()
        cb = manager.get_circuit_breaker("example.com")
        for _ in range(cb.failure_threshold):
            cb.record_failure()
        assert manager.is_circuit_open("example.com") is True

    def test_reset_circuit(self):
        """Test resetting a circuit breaker."""
        manager = NetworkManager()
        cb = manager.get_circuit_breaker("example.com")
        for _ in range(cb.failure_threshold):
            cb.record_failure()
        assert cb.state == CircuitState.OPEN

        manager.reset_circuit("example.com")
        assert cb.state == CircuitState.CLOSED

    def test_reset_nonexistent_circuit(self):
        """Test resetting nonexistent circuit doesn't error."""
        manager = NetworkManager()
        # Should not raise
        manager.reset_circuit("nonexistent.com")


@pytest.mark.unit
class TestNetworkManagerStats:
    """Tests for NetworkManager statistics."""

    def test_initial_stats(self):
        """Test initial stats are zero."""
        manager = NetworkManager()
        stats = manager.get_stats()
        assert stats.total_requests == 0

    def test_record_success(self):
        """Test recording successful request."""
        manager = NetworkManager()
        manager._record_success("example.com")
        stats = manager.get_stats()
        assert stats.total_requests == 1
        assert stats.successful_requests == 1

    def test_record_failure(self):
        """Test recording failed request."""
        manager = NetworkManager()
        manager._record_failure("example.com", ValueError("test"))
        stats = manager.get_stats()
        assert stats.total_requests == 1
        assert stats.failed_requests == 1

    def test_record_circuit_rejection(self):
        """Test recording circuit breaker rejection."""
        manager = NetworkManager()
        manager._record_circuit_rejection("example.com")
        stats = manager.get_stats()
        assert stats.circuit_breaker_rejections == 1

    def test_get_domain_stats(self):
        """Test getting stats for specific domain."""
        manager = NetworkManager()
        manager._record_success("example1.com")
        manager._record_success("example1.com")
        manager._record_failure("example2.com", ValueError())

        domain_stats = manager.get_domain_stats()
        assert domain_stats["example1.com"]["successes"] == 2
        assert domain_stats["example2.com"]["failures"] == 1


@pytest.mark.unit
class TestNetworkManagerRequests:
    """Tests for NetworkManager request functionality."""

    def test_request_success(self):
        """Test successful request."""
        manager = NetworkManager()

        def mock_request():
            return {"status": "ok"}

        result = manager.execute(mock_request, "https://example.com/api")
        assert result == {"status": "ok"}
        assert manager.get_stats().successful_requests == 1

    def test_request_with_retry(self):
        """Test request that succeeds after retry."""
        manager = NetworkManager(
            retry_config=RetryConfig(max_attempts=3, base_delay=0.01, jitter=False)
        )
        attempts = 0

        def mock_request():
            nonlocal attempts
            attempts += 1
            if attempts < 3:
                raise ConnectionError("Network error")
            return {"status": "ok"}

        result = manager.execute(
            mock_request, "https://example.com/api", retryable_exceptions=[ConnectionError]
        )
        assert result == {"status": "ok"}
        assert attempts == 3
        stats = manager.get_stats()
        assert stats.successful_requests == 1
        assert stats.retries == 2

    def test_request_circuit_open(self):
        """Test request rejected when circuit is open."""
        manager = NetworkManager()
        cb = manager.get_circuit_breaker("example.com")
        for _ in range(cb.failure_threshold):
            cb.record_failure()

        with pytest.raises(NetworkError) as exc_info:
            manager.execute(lambda: None, "https://example.com/api")

        assert "circuit breaker" in str(exc_info.value).lower()
        assert manager.get_stats().circuit_breaker_rejections == 1

    def test_request_exhausts_retries(self):
        """Test request that exhausts all retries."""
        manager = NetworkManager(
            retry_config=RetryConfig(max_attempts=3, base_delay=0.01, jitter=False)
        )

        def mock_request():
            raise ConnectionError("Network error")

        with pytest.raises(NetworkError) as exc_info:
            manager.execute(
                mock_request, "https://example.com/api", retryable_exceptions=[ConnectionError]
            )

        assert "3 attempts" in str(exc_info.value)
        stats = manager.get_stats()
        assert stats.failed_requests == 1
        assert stats.retries == 2


@pytest.mark.unit
class TestNetworkManagerAsync:
    """Tests for NetworkManager async functionality."""

    def test_async_request_success(self):
        """Test successful async request."""
        manager = NetworkManager()

        async def mock_request():
            return {"status": "ok"}

        async def run_test():
            return await manager.execute_async(mock_request, "https://example.com/api")

        result = asyncio.run(run_test())
        assert result == {"status": "ok"}
        assert manager.get_stats().successful_requests == 1

    def test_async_request_with_retry(self):
        """Test async request that succeeds after retry."""
        manager = NetworkManager(
            retry_config=RetryConfig(max_attempts=3, base_delay=0.01, jitter=False)
        )
        attempts = 0

        async def mock_request():
            nonlocal attempts
            attempts += 1
            if attempts < 3:
                raise ConnectionError("Network error")
            return {"status": "ok"}

        async def run_test():
            return await manager.execute_async(
                mock_request,
                "https://example.com/api",
                retryable_exceptions=[ConnectionError],
            )

        result = asyncio.run(run_test())
        assert result == {"status": "ok"}
        assert attempts == 3

    def test_async_request_circuit_open(self):
        """Test async request rejected when circuit is open."""
        manager = NetworkManager()
        cb = manager.get_circuit_breaker("example.com")
        for _ in range(cb.failure_threshold):
            cb.record_failure()

        async def mock_request():
            return {"status": "ok"}

        async def run_test():
            return await manager.execute_async(mock_request, "https://example.com/api")

        with pytest.raises(NetworkError) as exc_info:
            asyncio.run(run_test())

        assert "circuit breaker" in str(exc_info.value).lower()


@pytest.mark.unit
class TestNetworkManagerEdgeCases:
    """Edge case tests for NetworkManager."""

    def test_multiple_domains_independent(self):
        """Test that multiple domains have independent circuit breakers."""
        manager = NetworkManager()
        cb1 = manager.get_circuit_breaker("example1.com")
        cb2 = manager.get_circuit_breaker("example2.com")

        # Trip circuit for domain 1
        for _ in range(cb1.failure_threshold):
            cb1.record_failure()

        assert manager.is_circuit_open("example1.com") is True
        assert manager.is_circuit_open("example2.com") is False

    def test_get_all_circuit_states(self):
        """Test getting all circuit breaker states."""
        manager = NetworkManager()
        manager.get_circuit_breaker("example1.com")
        manager.get_circuit_breaker("example2.com")

        states = manager.get_all_circuit_states()
        assert "example1.com" in states
        assert "example2.com" in states
        assert states["example1.com"] == "closed"

    def test_clear_stats(self):
        """Test clearing statistics."""
        manager = NetworkManager()
        manager._record_success("example.com")
        manager._record_failure("example.com", ValueError())

        manager.clear_stats()
        stats = manager.get_stats()
        assert stats.total_requests == 0

    def test_request_with_custom_timeout(self):
        """Test request with custom timeout parameter."""
        manager = NetworkManager(default_timeout=30.0)

        # Just verify timeout is accessible
        assert manager.default_timeout == 30.0

    def test_non_retryable_exception_raises_immediately(self):
        """Test that non-retryable exceptions raise immediately."""
        manager = NetworkManager(retry_config=RetryConfig(max_attempts=3, base_delay=0.01))
        attempts = 0

        def mock_request():
            nonlocal attempts
            attempts += 1
            raise ValueError("Not retryable")

        with pytest.raises(ValueError):
            manager.execute(
                mock_request,
                "https://example.com/api",
                retryable_exceptions=[ConnectionError],
            )

        # Should only have attempted once
        assert attempts == 1
