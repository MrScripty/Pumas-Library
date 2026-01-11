"""TTL-based caching for HuggingFace API metadata.

Provides in-memory caching with time-to-live expiration to reduce
repeated API calls for the same model metadata.
"""

from __future__ import annotations

import threading
import time
from dataclasses import dataclass, field
from typing import Any, Dict, Optional

from backend.logging_config import get_logger

logger = get_logger(__name__)

# Default TTL: 5 minutes
_DEFAULT_TTL_SECONDS = 300


@dataclass
class CacheEntry:
    """A cached entry with expiration timestamp.

    Attributes:
        value: The cached value
        expires_at: Unix timestamp when this entry expires
    """

    value: Any
    expires_at: float


@dataclass
class HFMetadataCache:
    """TTL-based cache for HuggingFace model metadata.

    Caches model info, repo tree listings, and search results to
    reduce API calls during batch operations.

    Thread-safe for concurrent access.

    Attributes:
        ttl_seconds: Time-to-live in seconds for cache entries
        max_entries: Maximum number of entries to cache (LRU eviction)
    """

    ttl_seconds: int = _DEFAULT_TTL_SECONDS
    max_entries: int = 1000
    _cache: Dict[str, CacheEntry] = field(default_factory=dict)
    _lock: threading.Lock = field(default_factory=threading.Lock)
    _access_order: list[str] = field(default_factory=list)

    def get(self, key: str) -> Optional[Any]:
        """Get a cached value if it exists and hasn't expired.

        Args:
            key: Cache key

        Returns:
            Cached value if valid, None if expired or not found
        """
        with self._lock:
            entry = self._cache.get(key)
            if entry is None:
                return None

            now = time.time()
            if now > entry.expires_at:
                # Entry expired
                del self._cache[key]
                if key in self._access_order:
                    self._access_order.remove(key)
                return None

            # Update access order for LRU
            if key in self._access_order:
                self._access_order.remove(key)
            self._access_order.append(key)

            return entry.value

    def set(self, key: str, value: Any, ttl: Optional[int] = None) -> None:
        """Set a cached value with optional custom TTL.

        Args:
            key: Cache key
            value: Value to cache
            ttl: Custom TTL in seconds (uses default if not specified)
        """
        effective_ttl = ttl if ttl is not None else self.ttl_seconds
        expires_at = time.time() + effective_ttl

        with self._lock:
            # Evict oldest entries if at capacity
            while len(self._cache) >= self.max_entries:
                if self._access_order:
                    oldest_key = self._access_order.pop(0)
                    self._cache.pop(oldest_key, None)
                else:
                    break

            self._cache[key] = CacheEntry(value=value, expires_at=expires_at)

            # Update access order
            if key in self._access_order:
                self._access_order.remove(key)
            self._access_order.append(key)

    def invalidate(self, key: str) -> bool:
        """Invalidate a specific cache entry.

        Args:
            key: Cache key to invalidate

        Returns:
            True if entry was found and removed
        """
        with self._lock:
            if key in self._cache:
                del self._cache[key]
                if key in self._access_order:
                    self._access_order.remove(key)
                return True
            return False

    def invalidate_prefix(self, prefix: str) -> int:
        """Invalidate all entries with keys matching a prefix.

        Useful for invalidating all cached data for a specific repo.

        Args:
            prefix: Key prefix to match

        Returns:
            Number of entries invalidated
        """
        with self._lock:
            keys_to_remove = [k for k in self._cache.keys() if k.startswith(prefix)]
            for key in keys_to_remove:
                del self._cache[key]
                if key in self._access_order:
                    self._access_order.remove(key)
            return len(keys_to_remove)

    def clear(self) -> int:
        """Clear all cache entries.

        Returns:
            Number of entries cleared
        """
        with self._lock:
            count = len(self._cache)
            self._cache.clear()
            self._access_order.clear()
            return count

    def stats(self) -> Dict[str, Any]:
        """Get cache statistics.

        Returns:
            Dictionary with cache statistics
        """
        with self._lock:
            now = time.time()
            expired_count = sum(1 for entry in self._cache.values() if now > entry.expires_at)
            return {
                "entries": len(self._cache),
                "max_entries": self.max_entries,
                "expired_pending_cleanup": expired_count,
                "ttl_seconds": self.ttl_seconds,
            }

    def cleanup_expired(self) -> int:
        """Remove all expired entries.

        Returns:
            Number of entries removed
        """
        with self._lock:
            now = time.time()
            expired_keys = [k for k, entry in self._cache.items() if now > entry.expires_at]
            for key in expired_keys:
                del self._cache[key]
                if key in self._access_order:
                    self._access_order.remove(key)
            return len(expired_keys)


def make_cache_key(namespace: str, *args: Any) -> str:
    """Create a cache key from namespace and arguments.

    Args:
        namespace: Cache namespace (e.g., 'model_info', 'repo_tree')
        *args: Arguments to include in key

    Returns:
        Cache key string
    """
    parts = [namespace] + [str(arg) for arg in args if arg is not None]
    return ":".join(parts)


# Global cache instance
hf_metadata_cache = HFMetadataCache(
    ttl_seconds=_DEFAULT_TTL_SECONDS,
    max_entries=1000,
)
