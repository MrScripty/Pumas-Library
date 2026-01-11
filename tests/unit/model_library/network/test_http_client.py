"""Tests for the AsyncHttpClient with HTTP/2 support."""

from __future__ import annotations

import asyncio
from unittest.mock import AsyncMock, MagicMock, patch

import pytest


class TestRateLimitState:
    """Tests for RateLimitState class."""

    def test_initial_state(self) -> None:
        """Rate limit state starts with no data."""
        from backend.model_library.network.http_client import RateLimitState

        state = RateLimitState()
        assert state.remaining is None
        assert state.limit is None
        assert state.reset_time is None
        assert state.warning_active is False
        assert state.should_throttle is False

    def test_update_from_headers(self) -> None:
        """Headers update rate limit state."""
        from backend.model_library.network.http_client import RateLimitState

        state = RateLimitState()
        state.update_from_headers(
            {
                "X-RateLimit-Remaining": "50",
                "X-RateLimit-Limit": "100",
                "X-RateLimit-Reset": "1234567890.5",
            }
        )

        assert state.remaining == 50
        assert state.limit == 100
        assert state.reset_time == 1234567890.5
        assert state.should_throttle is False

    def test_throttle_when_low(self) -> None:
        """Should throttle when remaining is below 10%."""
        from backend.model_library.network.http_client import RateLimitState

        state = RateLimitState()
        state.update_from_headers(
            {
                "X-RateLimit-Remaining": "5",
                "X-RateLimit-Limit": "100",
            }
        )

        assert state.should_throttle is True
        assert state.warning_active is True

    def test_no_throttle_when_sufficient(self) -> None:
        """Should not throttle when remaining is above 10%."""
        from backend.model_library.network.http_client import RateLimitState

        state = RateLimitState()
        state.update_from_headers(
            {
                "X-RateLimit-Remaining": "50",
                "X-RateLimit-Limit": "100",
            }
        )

        assert state.should_throttle is False

    def test_warning_clears_above_50_percent(self) -> None:
        """Warning flag clears when remaining goes above 50%."""
        from backend.model_library.network.http_client import RateLimitState

        state = RateLimitState()

        # Trigger warning
        state.update_from_headers({"X-RateLimit-Remaining": "5", "X-RateLimit-Limit": "100"})
        assert state.warning_active is True

        # Clear warning
        state.update_from_headers({"X-RateLimit-Remaining": "60", "X-RateLimit-Limit": "100"})
        assert state.warning_active is False

    def test_invalid_header_values_ignored(self) -> None:
        """Invalid header values are gracefully ignored."""
        from backend.model_library.network.http_client import RateLimitState

        state = RateLimitState()
        state.update_from_headers(
            {
                "X-RateLimit-Remaining": "not-a-number",
                "X-RateLimit-Limit": "also-not-a-number",
            }
        )

        assert state.remaining is None
        assert state.limit is None

    def test_zero_limit_triggers_throttle(self) -> None:
        """Zero limit triggers throttle."""
        from backend.model_library.network.http_client import RateLimitState

        state = RateLimitState()
        state.remaining = 0
        state.limit = 0

        assert state.should_throttle is True


class TestAsyncHttpClient:
    """Tests for AsyncHttpClient class."""

    @pytest.fixture
    def mock_httpx(self) -> MagicMock:
        """Create a mock httpx module."""
        mock = MagicMock()
        mock.AsyncClient = MagicMock()
        mock.Limits = MagicMock()
        return mock

    def test_init_without_httpx_raises(self) -> None:
        """Raises RuntimeError if httpx is not available."""
        with patch("backend.model_library.network.http_client.HTTPX_AVAILABLE", False):
            from backend.model_library.network.http_client import AsyncHttpClient

            with pytest.raises(RuntimeError, match="httpx is not installed"):
                AsyncHttpClient()

    def test_get_client_creates_once(self) -> None:
        """Client is created only once."""

        async def run_test() -> None:
            with patch("backend.model_library.network.http_client.HTTPX_AVAILABLE", True):
                with patch("backend.model_library.network.http_client.httpx") as mock_httpx:
                    mock_client = AsyncMock()
                    mock_httpx.AsyncClient.return_value = mock_client
                    mock_httpx.Limits = MagicMock()

                    from backend.model_library.network.http_client import AsyncHttpClient

                    client = AsyncHttpClient()

                    # First call creates client
                    await client._get_client()
                    assert mock_httpx.AsyncClient.call_count == 1

                    # Second call reuses client
                    await client._get_client()
                    assert mock_httpx.AsyncClient.call_count == 1

        asyncio.run(run_test())

    def test_http2_enabled_by_default(self) -> None:
        """HTTP/2 is enabled by default."""

        async def run_test() -> None:
            with patch("backend.model_library.network.http_client.HTTPX_AVAILABLE", True):
                with patch("backend.model_library.network.http_client.httpx") as mock_httpx:
                    mock_httpx.AsyncClient.return_value = AsyncMock()
                    mock_httpx.Limits = MagicMock()

                    from backend.model_library.network.http_client import AsyncHttpClient

                    client = AsyncHttpClient()
                    assert client.is_http2_enabled is True

                    await client._get_client()

                    # Verify http2=True was passed
                    call_kwargs = mock_httpx.AsyncClient.call_args[1]
                    assert call_kwargs["http2"] is True

        asyncio.run(run_test())

    def test_request_updates_rate_limit(self) -> None:
        """Request updates rate limit state from response headers."""

        async def run_test() -> None:
            with patch("backend.model_library.network.http_client.HTTPX_AVAILABLE", True):
                with patch("backend.model_library.network.http_client.httpx") as mock_httpx:
                    mock_response = MagicMock()
                    mock_response.headers = {
                        "X-RateLimit-Remaining": "42",
                        "X-RateLimit-Limit": "100",
                    }

                    mock_client = AsyncMock()
                    mock_client.request.return_value = mock_response
                    mock_httpx.AsyncClient.return_value = mock_client
                    mock_httpx.Limits = MagicMock()

                    from backend.model_library.network.http_client import AsyncHttpClient

                    client = AsyncHttpClient()
                    await client.get("https://example.com")

                    assert client.rate_limit_state.remaining == 42
                    assert client.rate_limit_state.limit == 100

        asyncio.run(run_test())

    def test_close_releases_resources(self) -> None:
        """Close method releases client resources."""

        async def run_test() -> None:
            with patch("backend.model_library.network.http_client.HTTPX_AVAILABLE", True):
                with patch("backend.model_library.network.http_client.httpx") as mock_httpx:
                    mock_client = AsyncMock()
                    mock_httpx.AsyncClient.return_value = mock_client
                    mock_httpx.Limits = MagicMock()

                    from backend.model_library.network.http_client import AsyncHttpClient

                    client = AsyncHttpClient()
                    await client._get_client()
                    await client.close()

                    mock_client.aclose.assert_called_once()
                    assert client._client is None

        asyncio.run(run_test())

    def test_context_manager(self) -> None:
        """Context manager creates and closes client."""

        async def run_test() -> None:
            with patch("backend.model_library.network.http_client.HTTPX_AVAILABLE", True):
                with patch("backend.model_library.network.http_client.httpx") as mock_httpx:
                    mock_client = AsyncMock()
                    mock_httpx.AsyncClient.return_value = mock_client
                    mock_httpx.Limits = MagicMock()

                    from backend.model_library.network.http_client import AsyncHttpClient

                    async with AsyncHttpClient() as client:
                        await client._get_client()

                    mock_client.aclose.assert_called_once()

        asyncio.run(run_test())


class TestSharedClient:
    """Tests for shared client functions."""

    def test_get_shared_client_creates_once(self) -> None:
        """Shared client is created only once."""

        async def run_test() -> None:
            with patch("backend.model_library.network.http_client.HTTPX_AVAILABLE", True):
                with patch("backend.model_library.network.http_client.httpx") as mock_httpx:
                    mock_httpx.AsyncClient.return_value = AsyncMock()
                    mock_httpx.Limits = MagicMock()

                    # Reset shared client
                    import backend.model_library.network.http_client as module

                    module._shared_client = None

                    from backend.model_library.network.http_client import (
                        close_shared_client,
                        get_shared_client,
                    )

                    client1 = await get_shared_client()
                    client2 = await get_shared_client()

                    assert client1 is client2

                    # Cleanup
                    await close_shared_client()

        asyncio.run(run_test())

    def test_close_shared_client(self) -> None:
        """Closing shared client sets it to None."""

        async def run_test() -> None:
            with patch("backend.model_library.network.http_client.HTTPX_AVAILABLE", True):
                with patch("backend.model_library.network.http_client.httpx") as mock_httpx:
                    mock_client = AsyncMock()
                    mock_httpx.AsyncClient.return_value = mock_client
                    mock_httpx.Limits = MagicMock()

                    # Reset shared client
                    import backend.model_library.network.http_client as module

                    module._shared_client = None

                    from backend.model_library.network.http_client import (
                        close_shared_client,
                        get_shared_client,
                    )

                    await get_shared_client()
                    await close_shared_client()

                    assert module._shared_client is None

        asyncio.run(run_test())
