"""Tests for HuggingFace metadata cache."""

from __future__ import annotations

import threading
import time

import pytest

from backend.model_library.hf.cache import HFMetadataCache, hf_metadata_cache, make_cache_key


@pytest.mark.unit
def test_cache_set_and_get():
    """Test basic set and get operations."""
    cache = HFMetadataCache(ttl_seconds=60)
    cache.set("test_key", {"data": "value"})
    result = cache.get("test_key")
    assert result == {"data": "value"}


@pytest.mark.unit
def test_cache_get_nonexistent():
    """Test getting nonexistent key returns None."""
    cache = HFMetadataCache(ttl_seconds=60)
    result = cache.get("nonexistent")
    assert result is None


@pytest.mark.unit
def test_cache_expiration():
    """Test that entries expire after TTL."""
    cache = HFMetadataCache(ttl_seconds=1)
    cache.set("test_key", "value")

    # Should be available immediately
    assert cache.get("test_key") == "value"

    # Wait for expiration
    time.sleep(1.1)

    # Should be expired now
    assert cache.get("test_key") is None


@pytest.mark.unit
def test_cache_custom_ttl():
    """Test custom TTL per entry."""
    cache = HFMetadataCache(ttl_seconds=60)
    cache.set("long_key", "long_value", ttl=60)
    cache.set("short_key", "short_value", ttl=1)

    assert cache.get("long_key") == "long_value"
    assert cache.get("short_key") == "short_value"

    time.sleep(1.1)

    # Short TTL entry should be expired
    assert cache.get("long_key") == "long_value"
    assert cache.get("short_key") is None


@pytest.mark.unit
def test_cache_invalidate():
    """Test invalidating a specific key."""
    cache = HFMetadataCache(ttl_seconds=60)
    cache.set("key1", "value1")
    cache.set("key2", "value2")

    result = cache.invalidate("key1")
    assert result is True
    assert cache.get("key1") is None
    assert cache.get("key2") == "value2"


@pytest.mark.unit
def test_cache_invalidate_nonexistent():
    """Test invalidating nonexistent key returns False."""
    cache = HFMetadataCache(ttl_seconds=60)
    result = cache.invalidate("nonexistent")
    assert result is False


@pytest.mark.unit
def test_cache_invalidate_prefix():
    """Test invalidating by prefix."""
    cache = HFMetadataCache(ttl_seconds=60)
    cache.set("repo:owner/model1:info", "info1")
    cache.set("repo:owner/model1:tree", "tree1")
    cache.set("repo:owner/model2:info", "info2")

    count = cache.invalidate_prefix("repo:owner/model1")
    assert count == 2
    assert cache.get("repo:owner/model1:info") is None
    assert cache.get("repo:owner/model1:tree") is None
    assert cache.get("repo:owner/model2:info") == "info2"


@pytest.mark.unit
def test_cache_clear():
    """Test clearing all entries."""
    cache = HFMetadataCache(ttl_seconds=60)
    cache.set("key1", "value1")
    cache.set("key2", "value2")
    cache.set("key3", "value3")

    count = cache.clear()
    assert count == 3
    assert cache.get("key1") is None
    assert cache.get("key2") is None
    assert cache.get("key3") is None


@pytest.mark.unit
def test_cache_stats():
    """Test getting cache statistics."""
    cache = HFMetadataCache(ttl_seconds=60, max_entries=100)
    cache.set("key1", "value1")
    cache.set("key2", "value2")

    stats = cache.stats()
    assert stats["entries"] == 2
    assert stats["max_entries"] == 100
    assert stats["ttl_seconds"] == 60


@pytest.mark.unit
def test_cache_cleanup_expired():
    """Test cleaning up expired entries."""
    cache = HFMetadataCache(ttl_seconds=1)
    cache.set("key1", "value1")
    cache.set("key2", "value2")

    time.sleep(1.1)

    # Entries are still in cache until cleanup
    count = cache.cleanup_expired()
    assert count == 2


@pytest.mark.unit
def test_cache_lru_eviction():
    """Test LRU eviction when at max capacity."""
    cache = HFMetadataCache(ttl_seconds=60, max_entries=3)
    cache.set("key1", "value1")
    cache.set("key2", "value2")
    cache.set("key3", "value3")

    # Access key1 to make it recently used
    cache.get("key1")

    # Add new entry, should evict key2 (least recently used)
    cache.set("key4", "value4")

    assert cache.get("key1") == "value1"  # Recently accessed
    assert cache.get("key2") is None  # Evicted
    assert cache.get("key3") == "value3"
    assert cache.get("key4") == "value4"


@pytest.mark.unit
def test_cache_thread_safety():
    """Test that cache is thread-safe."""
    cache = HFMetadataCache(ttl_seconds=60, max_entries=1000)
    errors = []

    def writer():
        try:
            for i in range(100):
                cache.set(f"writer_key_{i}", f"value_{i}")
        except RuntimeError as e:  # noqa: no-except-logging
            errors.append(e)

    def reader():
        try:
            for i in range(100):
                cache.get(f"writer_key_{i}")
        except RuntimeError as e:  # noqa: no-except-logging
            errors.append(e)

    threads = []
    for _ in range(5):
        threads.append(threading.Thread(target=writer))
        threads.append(threading.Thread(target=reader))

    for t in threads:
        t.start()
    for t in threads:
        t.join()

    assert len(errors) == 0


@pytest.mark.unit
def test_make_cache_key():
    """Test cache key generation."""
    key = make_cache_key("model_info", "owner/repo")
    assert key == "model_info:owner/repo"

    key = make_cache_key("repo_tree", "owner/repo", "main")
    assert key == "repo_tree:owner/repo:main"


@pytest.mark.unit
def test_make_cache_key_ignores_none():
    """Test that None values are ignored in key."""
    key = make_cache_key("search", "query", None, "filter")
    assert key == "search:query:filter"


@pytest.mark.unit
def test_global_cache_instance():
    """Test global cache instance exists."""
    assert hf_metadata_cache is not None
    assert isinstance(hf_metadata_cache, HFMetadataCache)
    assert hf_metadata_cache.ttl_seconds == 300  # Default 5 minutes
