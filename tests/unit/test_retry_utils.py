#!/usr/bin/env python3
"""
Unit tests for retry utilities with exponential backoff
"""

import time
from unittest.mock import MagicMock

import pytest

from backend.retry_utils import calculate_backoff_delay, retry_operation, retry_with_backoff


class TestCalculateBackoffDelay:
    """Test exponential backoff delay calculation"""

    def test_first_retry_delay(self):
        """First retry should use base_delay^1"""
        delay = calculate_backoff_delay(0, base_delay=2.0, jitter=False)
        assert delay == 2.0

    def test_second_retry_delay(self):
        """Second retry should use base_delay^2"""
        delay = calculate_backoff_delay(1, base_delay=2.0, jitter=False)
        assert delay == 4.0

    def test_third_retry_delay(self):
        """Third retry should use base_delay^3"""
        delay = calculate_backoff_delay(2, base_delay=2.0, jitter=False)
        assert delay == 8.0

    def test_max_delay_cap(self):
        """Delay should be capped at max_delay"""
        # 2^10 = 1024, but max_delay=60
        delay = calculate_backoff_delay(10, base_delay=2.0, max_delay=60.0, jitter=False)
        assert delay == 60.0

    def test_jitter_adds_randomness(self):
        """Jitter should add 0-1 seconds"""
        # Run multiple times to ensure jitter varies
        delays = [calculate_backoff_delay(0, base_delay=2.0, jitter=True) for _ in range(10)]

        # All delays should be between 2.0 and 3.0
        assert all(2.0 <= d <= 3.0 for d in delays)

        # At least some delays should be different (jitter working)
        assert len(set(delays)) > 1

    def test_custom_base_delay(self):
        """Should support custom base delay"""
        delay = calculate_backoff_delay(0, base_delay=3.0, jitter=False)
        assert delay == 3.0

        delay = calculate_backoff_delay(1, base_delay=3.0, jitter=False)
        assert delay == 9.0


class TestRetryWithBackoff:
    """Test retry_with_backoff function"""

    def test_success_on_first_try(self):
        """Should return immediately if function succeeds"""
        mock_func = MagicMock(return_value="success")
        result = retry_with_backoff(mock_func, max_retries=3)

        assert result == "success"
        assert mock_func.call_count == 1

    def test_success_on_second_try(self):
        """Should retry once then succeed"""
        mock_func = MagicMock(side_effect=[ValueError("fail"), "success"])
        result = retry_with_backoff(mock_func, max_retries=3, base_delay=0.01)

        assert result == "success"
        assert mock_func.call_count == 2

    def test_all_retries_fail(self):
        """Should return None after all retries fail"""
        mock_func = MagicMock(side_effect=ValueError("fail"))
        result = retry_with_backoff(mock_func, max_retries=2, base_delay=0.01)

        assert result is None
        assert mock_func.call_count == 3  # 1 initial + 2 retries

    def test_on_retry_callback(self):
        """Should call on_retry callback before each retry"""
        mock_func = MagicMock(side_effect=[ValueError("fail"), ValueError("fail"), "success"])
        callback = MagicMock()

        result = retry_with_backoff(mock_func, max_retries=3, base_delay=0.01, on_retry=callback)

        assert result == "success"
        assert callback.call_count == 2  # Called before 2nd and 3rd attempts

        # Check callback arguments
        first_call = callback.call_args_list[0]
        assert first_call[0][0] == 0  # attempt number
        assert isinstance(first_call[0][1], float)  # delay
        assert isinstance(first_call[0][2], ValueError)  # exception

    def test_specific_exception_filtering(self):
        """Should only retry on specified exceptions"""
        mock_func = MagicMock(side_effect=RuntimeError("fail"))

        # Should not retry RuntimeError when only catching ValueError
        # The function will raise the unhandled exception
        with pytest.raises(RuntimeError):
            retry_with_backoff(mock_func, max_retries=3, base_delay=0.01, exceptions=(ValueError,))

        # Should fail immediately without retries
        assert mock_func.call_count == 1

    def test_backoff_timing(self):
        """Should wait with exponential backoff between retries"""
        mock_func = MagicMock(side_effect=[ValueError("fail"), ValueError("fail"), "success"])

        start = time.time()
        retry_with_backoff(mock_func, max_retries=3, base_delay=0.1)
        elapsed = time.time() - start

        # Should have waited ~0.1s + ~0.2s = ~0.3s total (with jitter, allow generous margin for CI)
        assert 0.2 < elapsed < 2.0


class TestRetryOperation:
    """Test retry_operation convenience function"""

    def test_success_returns_true(self):
        """Should return True when operation succeeds"""
        mock_op = MagicMock(return_value=True)
        result = retry_operation(mock_op, max_retries=3, base_delay=0.01)

        assert result is True
        assert mock_op.call_count == 1

    def test_failure_returns_false(self):
        """Should return False when operation always fails"""
        mock_op = MagicMock(return_value=False)
        result = retry_operation(mock_op, max_retries=2, base_delay=0.01, operation_name="TestOp")

        assert result is False
        assert mock_op.call_count == 3  # 1 initial + 2 retries

    def test_eventual_success(self):
        """Should succeed on retry"""
        mock_op = MagicMock(side_effect=[False, False, True])
        result = retry_operation(mock_op, max_retries=3, base_delay=0.01)

        assert result is True
        assert mock_op.call_count == 3

    def test_exception_handling(self):
        """Should handle exceptions raised by operation"""
        mock_op = MagicMock(side_effect=[Exception("error"), True])
        result = retry_operation(mock_op, max_retries=3, base_delay=0.01)

        assert result is True
        assert mock_op.call_count == 2


class TestRetryIntegration:
    """Integration tests for retry logic"""

    def test_realistic_network_retry_scenario(self):
        """Simulate network failure then recovery"""
        call_count = 0

        def unreliable_network_call():
            nonlocal call_count
            call_count += 1
            if call_count < 3:
                raise ConnectionError("Network unavailable")
            return "success"

        result = retry_with_backoff(unreliable_network_call, max_retries=5, base_delay=0.01)

        assert result == "success"
        assert call_count == 3

    def test_timeout_not_retried_indefinitely(self):
        """Should respect max_retries limit"""

        def always_timeout():
            raise TimeoutError("Request timeout")

        start = time.time()
        result = retry_with_backoff(always_timeout, max_retries=2, base_delay=0.05)
        elapsed = time.time() - start

        assert result is None
        # Should fail quickly (not hang forever) - allow generous margin for CI
        assert elapsed < 2.0


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
