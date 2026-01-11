"""Tests for HuggingFace API throttle."""

from __future__ import annotations

import threading
import time
from unittest.mock import patch

import pytest

from backend.model_library.hf.throttle import HFAPIThrottle, hf_throttle


@pytest.mark.unit
def test_throttle_init_defaults():
    """Test HFAPIThrottle initialization with defaults."""
    throttle = HFAPIThrottle()
    assert throttle.max_calls == 60
    assert throttle.window_seconds == 60


@pytest.mark.unit
def test_throttle_init_custom():
    """Test HFAPIThrottle initialization with custom values."""
    throttle = HFAPIThrottle(max_calls_per_minute=30)
    assert throttle.max_calls == 30


@pytest.mark.unit
def test_acquire_records_timestamp():
    """Test that acquire records call timestamp."""
    throttle = HFAPIThrottle(max_calls_per_minute=10)
    assert len(throttle._call_timestamps) == 0

    throttle.acquire()
    assert len(throttle._call_timestamps) == 1


@pytest.mark.unit
def test_get_remaining_calls():
    """Test get_remaining_calls returns correct count."""
    throttle = HFAPIThrottle(max_calls_per_minute=5)
    assert throttle.get_remaining_calls() == 5

    throttle.acquire()
    assert throttle.get_remaining_calls() == 4

    throttle.acquire()
    throttle.acquire()
    assert throttle.get_remaining_calls() == 2


@pytest.mark.unit
def test_is_rate_limited_under_capacity():
    """Test is_rate_limited returns False when under capacity."""
    throttle = HFAPIThrottle(max_calls_per_minute=10)
    assert throttle.is_rate_limited() is False

    throttle.acquire()
    assert throttle.is_rate_limited() is False


@pytest.mark.unit
def test_is_rate_limited_at_capacity():
    """Test is_rate_limited returns True at capacity."""
    throttle = HFAPIThrottle(max_calls_per_minute=3)

    for _ in range(3):
        throttle.acquire()

    assert throttle.is_rate_limited() is True


@pytest.mark.unit
def test_set_backoff():
    """Test set_backoff sets the backoff timestamp."""
    throttle = HFAPIThrottle(max_calls_per_minute=10)
    assert throttle._backoff_until is None

    throttle.set_backoff(5)
    assert throttle._backoff_until is not None
    assert throttle._backoff_until > time.time()


@pytest.mark.unit
def test_is_rate_limited_during_backoff():
    """Test is_rate_limited returns True during backoff."""
    throttle = HFAPIThrottle(max_calls_per_minute=10)
    throttle.set_backoff(10)
    assert throttle.is_rate_limited() is True


@pytest.mark.unit
def test_reset_clears_state():
    """Test reset clears all state."""
    throttle = HFAPIThrottle(max_calls_per_minute=5)

    # Add some state
    throttle.acquire()
    throttle.acquire()
    throttle.set_backoff(60)

    assert len(throttle._call_timestamps) == 2
    assert throttle._backoff_until is not None

    # Reset
    throttle.reset()

    assert len(throttle._call_timestamps) == 0
    assert throttle._backoff_until is None


@pytest.mark.unit
def test_global_hf_throttle_exists():
    """Test global hf_throttle instance exists."""
    assert hf_throttle is not None
    assert isinstance(hf_throttle, HFAPIThrottle)
    assert hf_throttle.max_calls == 60


@pytest.mark.unit
def test_thread_safety():
    """Test that throttle is thread-safe."""
    throttle = HFAPIThrottle(max_calls_per_minute=100)
    errors = []

    def worker():
        try:
            for _ in range(10):
                throttle.acquire()
        except RuntimeError as e:  # noqa: no-except-logging
            errors.append(e)

    threads = [threading.Thread(target=worker) for _ in range(5)]
    for t in threads:
        t.start()
    for t in threads:
        t.join()

    assert len(errors) == 0
    # All 50 calls should be recorded
    assert len(throttle._call_timestamps) == 50


@pytest.mark.unit
def test_sliding_window_clears_old_timestamps():
    """Test that old timestamps are cleared from sliding window."""
    throttle = HFAPIThrottle(max_calls_per_minute=10)

    # Manually add old timestamps (older than window)
    old_time = time.time() - 120  # 2 minutes ago
    throttle._call_timestamps = [old_time, old_time + 1]

    # Acquire should clear old and add new
    throttle.acquire()

    # Old timestamps should be cleared, only new one remains
    assert len(throttle._call_timestamps) == 1
    assert throttle._call_timestamps[0] > old_time + 60


@pytest.mark.unit
def test_acquire_waits_during_backoff():
    """Test that acquire waits when in backoff period."""
    throttle = HFAPIThrottle(max_calls_per_minute=10)

    # Set a short backoff
    throttle._backoff_until = time.time() + 0.05  # 50ms

    with patch("time.sleep") as mock_sleep:
        mock_sleep.return_value = None
        # After sleep, time should have "passed"
        throttle.acquire()

        # Should have called sleep
        assert mock_sleep.called
        # Backoff should be cleared
        assert throttle._backoff_until is None


@pytest.mark.unit
def test_acquire_waits_at_capacity():
    """Test that acquire waits when at capacity."""
    throttle = HFAPIThrottle(max_calls_per_minute=2)

    # Fill to capacity with timestamps that won't expire soon
    now = time.time()
    throttle._call_timestamps = [now - 1, now]  # 2 recent calls

    with patch("time.sleep") as mock_sleep:
        mock_sleep.return_value = None
        throttle.acquire()

        # Should have called sleep (waiting for oldest to expire)
        assert mock_sleep.called


@pytest.mark.unit
def test_acquire_no_wait_when_timestamps_expired():
    """Test that acquire doesn't wait when timestamps have expired."""
    throttle = HFAPIThrottle(max_calls_per_minute=2)

    # Fill to capacity with OLD timestamps (older than window)
    old_time = time.time() - 120
    throttle._call_timestamps = [old_time, old_time + 1]

    start = time.time()
    throttle.acquire()
    elapsed = time.time() - start

    # Should not have waited (< 100ms)
    assert elapsed < 0.1
    # Old timestamps should be cleared
    assert len(throttle._call_timestamps) == 1
