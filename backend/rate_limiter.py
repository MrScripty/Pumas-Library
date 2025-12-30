"""Simple in-memory rate limiter for API actions."""

from __future__ import annotations

import threading
import time
from collections import defaultdict, deque
from typing import DefaultDict, Deque


class RateLimiter:
    """Fixed-window rate limiter per key."""

    def __init__(self, max_calls: int, period_seconds: int) -> None:
        self.max_calls = max_calls
        self.period_seconds = period_seconds
        self._calls: DefaultDict[str, Deque[float]] = defaultdict(deque)
        self._lock = threading.Lock()

    def is_allowed(self, key: str) -> bool:
        now = time.monotonic()
        cutoff = now - self.period_seconds
        with self._lock:
            calls = self._calls[key]
            while calls and calls[0] < cutoff:
                calls.popleft()
            if len(calls) >= self.max_calls:
                return False
            calls.append(now)
            return True
