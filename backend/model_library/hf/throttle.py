"""HuggingFace API rate limiting and throttling.

Provides global rate limiting for HuggingFace API calls to prevent
hitting rate limits during batch operations.
"""

from __future__ import annotations

import threading
import time
from typing import Optional

from backend.logging_config import get_logger

logger = get_logger(__name__)

# Default: HuggingFace allows ~60 requests per minute for authenticated users
_DEFAULT_MAX_CALLS_PER_MINUTE = 60


class HFAPIThrottle:
    """Global rate limiter for HuggingFace API calls.

    Uses a sliding window algorithm to track API call timestamps and
    enforce rate limits. Supports automatic backoff when 429 responses
    are received.

    Attributes:
        max_calls: Maximum calls allowed per window
        window_seconds: Size of the sliding window in seconds
    """

    def __init__(
        self,
        max_calls_per_minute: int = _DEFAULT_MAX_CALLS_PER_MINUTE,
    ) -> None:
        """Initialize the throttle.

        Args:
            max_calls_per_minute: Maximum API calls per minute
        """
        self.max_calls = max_calls_per_minute
        self.window_seconds = 60
        self._call_timestamps: list[float] = []
        self._lock = threading.Lock()
        self._backoff_until: Optional[float] = None

    def acquire(self) -> None:
        """Wait until an API call is allowed under the rate limit.

        Blocks if:
        - Currently in a backoff period (from a 429 response)
        - At capacity for the current sliding window
        """
        with self._lock:
            now = time.time()

            # Check if in backoff period
            if self._backoff_until and now < self._backoff_until:
                wait_time = self._backoff_until - now
                logger.info("API rate limit: Waiting %.1fs (backoff)", wait_time)
                # Release lock while sleeping
                self._lock.release()
                time.sleep(wait_time)
                self._lock.acquire()
                now = time.time()
                self._backoff_until = None

            # Remove timestamps outside the window
            cutoff = now - self.window_seconds
            self._call_timestamps = [ts for ts in self._call_timestamps if ts > cutoff]

            # Wait if at capacity
            if len(self._call_timestamps) >= self.max_calls:
                oldest = self._call_timestamps[0]
                wait_time = (oldest + self.window_seconds) - now
                if wait_time > 0:
                    logger.debug("API throttle: Waiting %.1fs", wait_time)
                    # Release lock while sleeping
                    self._lock.release()
                    time.sleep(wait_time)
                    self._lock.acquire()
                    now = time.time()
                    # Clean up timestamps again after wait
                    cutoff = now - self.window_seconds
                    self._call_timestamps = [ts for ts in self._call_timestamps if ts > cutoff]

            # Record this call
            self._call_timestamps.append(now)

    def set_backoff(self, retry_after: int) -> None:
        """Set a backoff period from a 429 response.

        Args:
            retry_after: Seconds to wait before retrying (from Retry-After header)
        """
        with self._lock:
            self._backoff_until = time.time() + retry_after
            logger.warning(
                "HF API rate limit hit. Backing off for %ds",
                retry_after,
            )

    def get_remaining_calls(self) -> int:
        """Get the number of API calls remaining in the current window.

        Returns:
            Number of calls that can be made without waiting
        """
        with self._lock:
            now = time.time()
            cutoff = now - self.window_seconds
            active_calls = sum(1 for ts in self._call_timestamps if ts > cutoff)
            return max(0, self.max_calls - active_calls)

    def is_rate_limited(self) -> bool:
        """Check if currently rate limited (in backoff or at capacity).

        Returns:
            True if API calls would need to wait
        """
        with self._lock:
            now = time.time()

            # Check backoff
            if self._backoff_until and now < self._backoff_until:
                return True

            # Check capacity
            cutoff = now - self.window_seconds
            active_calls = sum(1 for ts in self._call_timestamps if ts > cutoff)
            return active_calls >= self.max_calls

    def reset(self) -> None:
        """Reset the throttle state (for testing)."""
        with self._lock:
            self._call_timestamps.clear()
            self._backoff_until = None


# Global instance for shared rate limiting across the application
hf_throttle = HFAPIThrottle(max_calls_per_minute=_DEFAULT_MAX_CALLS_PER_MINUTE)
