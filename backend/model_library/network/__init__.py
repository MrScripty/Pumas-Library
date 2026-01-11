"""Network operations for model library."""  # pragma: no cover

from __future__ import annotations  # pragma: no cover

from backend.model_library.network.circuit_breaker import (  # pragma: no cover
    CircuitBreaker,
    CircuitState,
)
from backend.model_library.network.http_client import (  # pragma: no cover
    AsyncHttpClient,
    RateLimitState,
    close_shared_client,
    get_shared_client,
)
from backend.model_library.network.manager import (  # pragma: no cover
    NetworkError,
    NetworkManager,
    NetworkStats,
    extract_domain,
)
from backend.model_library.network.retry import (  # pragma: no cover
    RetryConfig,
    RetryError,
    RetryStats,
    calculate_backoff,
    retry,
    retry_async,
    should_retry,
)

__all__ = [  # pragma: no cover
    # Circuit Breaker
    "CircuitBreaker",
    "CircuitState",
    # HTTP Client (Phase 4B)
    "AsyncHttpClient",
    "RateLimitState",
    "get_shared_client",
    "close_shared_client",
    # Manager
    "NetworkError",
    "NetworkManager",
    "NetworkStats",
    "extract_domain",
    # Retry
    "RetryConfig",
    "RetryError",
    "RetryStats",
    "calculate_backoff",
    "retry",
    "retry_async",
    "should_retry",
]
