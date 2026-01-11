"""Async HTTP client with HTTP/2 support and rate limiting.

Provides a shared httpx client for the model library with:
- HTTP/2 multiplexing for concurrent requests
- Rate limit header detection and proactive throttling
- Connection pooling for efficiency
"""

from __future__ import annotations

import asyncio
from typing import Any

from backend.logging_config import get_logger

logger = get_logger(__name__)

# Optional httpx import - graceful degradation if not installed
try:
    import httpx

    HTTPX_AVAILABLE = True
except ImportError:  # noqa: no-except-logging
    HTTPX_AVAILABLE = False
    httpx = None  # type: ignore[assignment]


class RateLimitState:
    """Tracks rate limit state across requests."""

    def __init__(self) -> None:
        self.remaining: int | None = None
        self.limit: int | None = None
        self.reset_time: float | None = None
        self.warning_active: bool = False

    def update_from_headers(self, headers: dict[str, str]) -> None:
        """Update rate limit state from response headers.

        Args:
            headers: Response headers dict
        """
        if "X-RateLimit-Remaining" in headers:
            try:
                self.remaining = int(headers["X-RateLimit-Remaining"])
            except ValueError:  # noqa: no-except-logging
                pass

        if "X-RateLimit-Limit" in headers:
            try:
                self.limit = int(headers["X-RateLimit-Limit"])
            except ValueError:  # noqa: no-except-logging
                pass

        if "X-RateLimit-Reset" in headers:
            try:
                self.reset_time = float(headers["X-RateLimit-Reset"])
            except ValueError:  # noqa: no-except-logging
                pass

        # Check if we should throttle
        if self.remaining is not None and self.limit is not None and self.limit > 0:
            ratio = self.remaining / self.limit
            if ratio < 0.1:
                if not self.warning_active:
                    logger.warning(
                        "Rate limit low (%d/%d, %.1f%%), throttling requests",
                        self.remaining,
                        self.limit,
                        ratio * 100,
                    )
                    self.warning_active = True
            elif ratio > 0.5:
                self.warning_active = False

    @property
    def should_throttle(self) -> bool:
        """Check if requests should be throttled."""
        if self.remaining is None or self.limit is None:
            return False
        if self.limit == 0:
            return True
        return (self.remaining / self.limit) < 0.1


class AsyncHttpClient:
    """Async HTTP client with HTTP/2 support and rate limiting.

    This client provides:
    - HTTP/2 multiplexing for concurrent requests over single TCP connection
    - Rate limit header detection with proactive throttling
    - Automatic retries with exponential backoff (via NetworkManager)
    - Connection pooling and keep-alive

    Usage:
        async with AsyncHttpClient() as client:
            response = await client.get("https://api.example.com/data")
    """

    def __init__(
        self,
        timeout: float = 7.0,
        http2: bool = True,
        max_connections: int = 100,
        max_keepalive_connections: int = 20,
    ) -> None:
        """Initialize the async HTTP client.

        Args:
            timeout: Default request timeout in seconds
            http2: Whether to enable HTTP/2 (requires h2 package)
            max_connections: Maximum total connections
            max_keepalive_connections: Maximum keep-alive connections
        """
        if not HTTPX_AVAILABLE:
            raise RuntimeError("httpx is not installed. Install with: pip install httpx[http2]")

        self._timeout = timeout
        self._http2 = http2
        self._max_connections = max_connections
        self._max_keepalive = max_keepalive_connections
        self._client: httpx.AsyncClient | None = None
        self._rate_limit = RateLimitState()
        self._throttle_delay = 0.5  # seconds to wait when throttled

    async def _get_client(self) -> httpx.AsyncClient:
        """Get or create the httpx client.

        Returns:
            Configured AsyncClient instance
        """
        if self._client is None:
            limits = httpx.Limits(
                max_connections=self._max_connections,
                max_keepalive_connections=self._max_keepalive,
            )
            self._client = httpx.AsyncClient(
                http2=self._http2,
                timeout=self._timeout,
                limits=limits,
                follow_redirects=True,
            )
            logger.debug(
                "Created httpx client (http2=%s, timeout=%.1fs)",
                self._http2,
                self._timeout,
            )
        return self._client

    async def _check_throttle(self) -> None:
        """Apply throttle delay if rate limited."""
        if self._rate_limit.should_throttle:
            logger.debug("Applying throttle delay of %.1fs", self._throttle_delay)
            await asyncio.sleep(self._throttle_delay)

    def _process_response(self, response: httpx.Response) -> None:
        """Process response headers for rate limiting.

        Args:
            response: httpx response object
        """
        self._rate_limit.update_from_headers(dict(response.headers))

    async def request(
        self,
        method: str,
        url: str,
        **kwargs: Any,
    ) -> httpx.Response:
        """Make an HTTP request.

        Args:
            method: HTTP method (GET, POST, etc.)
            url: Request URL
            **kwargs: Additional arguments passed to httpx

        Returns:
            httpx Response object

        Raises:
            httpx.TimeoutException: On request timeout
            httpx.ConnectError: On connection failure
            httpx.HTTPStatusError: On HTTP error status (if raise_for_status called)
        """
        await self._check_throttle()

        client = await self._get_client()
        response = await client.request(method, url, **kwargs)
        self._process_response(response)

        return response

    async def get(self, url: str, **kwargs: Any) -> httpx.Response:
        """Make a GET request.

        Args:
            url: Request URL
            **kwargs: Additional arguments passed to httpx

        Returns:
            httpx Response object
        """
        return await self.request("GET", url, **kwargs)

    async def post(self, url: str, **kwargs: Any) -> httpx.Response:
        """Make a POST request.

        Args:
            url: Request URL
            **kwargs: Additional arguments passed to httpx

        Returns:
            httpx Response object
        """
        return await self.request("POST", url, **kwargs)

    async def head(self, url: str, **kwargs: Any) -> httpx.Response:
        """Make a HEAD request.

        Args:
            url: Request URL
            **kwargs: Additional arguments passed to httpx

        Returns:
            httpx Response object
        """
        return await self.request("HEAD", url, **kwargs)

    @property
    def rate_limit_state(self) -> RateLimitState:
        """Get the current rate limit state."""
        return self._rate_limit

    @property
    def is_http2_enabled(self) -> bool:
        """Check if HTTP/2 is enabled."""
        return self._http2

    async def close(self) -> None:
        """Close the HTTP client and release resources."""
        if self._client is not None:
            await self._client.aclose()
            self._client = None
            logger.debug("Closed httpx client")

    async def __aenter__(self) -> "AsyncHttpClient":
        """Async context manager entry."""
        return self

    async def __aexit__(self, *args: Any) -> None:
        """Async context manager exit."""
        await self.close()


# Module-level client instance for reuse
_shared_client: AsyncHttpClient | None = None


async def get_shared_client() -> AsyncHttpClient:
    """Get the shared HTTP client instance.

    Returns:
        Shared AsyncHttpClient instance

    Note:
        The shared client is created on first use and reused for efficiency.
        Call close_shared_client() when shutting down to release resources.
    """
    global _shared_client
    if _shared_client is None:
        _shared_client = AsyncHttpClient()
    return _shared_client


async def close_shared_client() -> None:
    """Close the shared HTTP client and release resources."""
    global _shared_client
    if _shared_client is not None:
        await _shared_client.close()
        _shared_client = None
