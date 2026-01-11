"""Tests for stream hashing utilities."""

from __future__ import annotations

import hashlib
import tempfile
from pathlib import Path

import pytest

from backend.model_library.io.hashing import (
    StreamHasher,
    compute_dual_hash,
    hash_file_blake3,
    hash_file_sha256,
)


@pytest.mark.unit
class TestStreamHasher:
    """Tests for StreamHasher class."""

    def test_init_default_algorithms(self):
        """Test default initialization with sha256 and blake3."""
        hasher = StreamHasher()
        assert "sha256" in hasher.hashers
        assert "blake3" in hasher.hashers

    def test_init_custom_algorithms(self):
        """Test initialization with custom algorithms."""
        hasher = StreamHasher(algorithms=["sha256", "md5"])
        assert "sha256" in hasher.hashers
        assert "md5" in hasher.hashers
        assert "blake3" not in hasher.hashers

    def test_update_single_chunk(self):
        """Test updating with a single chunk."""
        hasher = StreamHasher(algorithms=["sha256"])
        data = b"test data"
        hasher.update(data)
        result = hasher.hexdigest()

        # Verify against direct hashlib computation
        expected = hashlib.sha256(data).hexdigest()
        assert result["sha256"] == expected

    def test_update_multiple_chunks(self):
        """Test updating with multiple chunks."""
        hasher = StreamHasher(algorithms=["sha256"])
        chunks = [b"chunk1", b"chunk2", b"chunk3"]

        for chunk in chunks:
            hasher.update(chunk)

        result = hasher.hexdigest()

        # Verify against direct hashlib computation
        expected = hashlib.sha256(b"".join(chunks)).hexdigest()
        assert result["sha256"] == expected

    def test_update_empty_chunk(self):
        """Test that empty chunks don't affect hash."""
        hasher = StreamHasher(algorithms=["sha256"])
        hasher.update(b"data")
        hasher.update(b"")
        hasher.update(b"more")

        result = hasher.hexdigest()
        expected = hashlib.sha256(b"datamore").hexdigest()
        assert result["sha256"] == expected

    def test_hexdigest_returns_all_hashes(self):
        """Test that hexdigest returns all configured hashes."""
        hasher = StreamHasher(algorithms=["sha256", "md5"])
        hasher.update(b"test")
        result = hasher.hexdigest()

        assert "sha256" in result
        assert "md5" in result
        assert len(result) == 2

    def test_dual_hash_sha256_and_blake3(self):
        """Test computing both sha256 and blake3 simultaneously."""
        hasher = StreamHasher(algorithms=["sha256", "blake3"])
        data = b"test data for dual hash"
        hasher.update(data)
        result = hasher.hexdigest()

        assert "sha256" in result
        assert "blake3" in result
        assert len(result["sha256"]) == 64  # SHA256 produces 64 hex chars
        assert len(result["blake3"]) == 64  # BLAKE3 produces 64 hex chars


@pytest.mark.unit
class TestHashFileSha256:
    """Tests for hash_file_sha256 function."""

    def test_hash_small_file(self, tmp_path: Path):
        """Test hashing a small file."""
        test_file = tmp_path / "small.txt"
        content = b"small file content"
        test_file.write_bytes(content)

        result = hash_file_sha256(test_file)
        expected = hashlib.sha256(content).hexdigest()
        assert result == expected

    def test_hash_large_file(self, tmp_path: Path):
        """Test hashing a large file (>8MB) to verify chunked reading."""
        test_file = tmp_path / "large.bin"
        # Create 10MB file
        chunk_size = 1024 * 1024  # 1MB
        chunks = 10
        content = b"x" * chunk_size

        with test_file.open("wb") as f:
            for _ in range(chunks):
                f.write(content)

        result = hash_file_sha256(test_file)

        # Verify it matches a streamed hash
        hasher = hashlib.sha256()
        with test_file.open("rb") as f:
            for chunk in iter(lambda: f.read(8192 * 1024), b""):
                hasher.update(chunk)
        expected = hasher.hexdigest()

        assert result == expected

    def test_hash_empty_file(self, tmp_path: Path):
        """Test hashing an empty file."""
        test_file = tmp_path / "empty.txt"
        test_file.touch()

        result = hash_file_sha256(test_file)
        expected = hashlib.sha256(b"").hexdigest()
        assert result == expected

    def test_hash_nonexistent_file(self, tmp_path: Path):
        """Test that hashing nonexistent file raises FileNotFoundError."""
        test_file = tmp_path / "nonexistent.txt"

        with pytest.raises(FileNotFoundError):
            hash_file_sha256(test_file)


@pytest.mark.unit
class TestHashFileBlake3:
    """Tests for hash_file_blake3 function."""

    def test_hash_small_file(self, tmp_path: Path):
        """Test hashing a small file with BLAKE3."""
        try:
            import blake3
        except ImportError:  # noqa: no-except-logging
            pytest.skip("blake3 not available")

        test_file = tmp_path / "small.txt"
        content = b"small file content"
        test_file.write_bytes(content)

        result = hash_file_blake3(test_file)
        expected = blake3.blake3(content).hexdigest()
        assert result == expected

    def test_hash_large_file(self, tmp_path: Path):
        """Test hashing a large file with BLAKE3."""
        try:
            import blake3
        except ImportError:  # noqa: no-except-logging
            pytest.skip("blake3 not available")

        test_file = tmp_path / "large.bin"
        chunk_size = 1024 * 1024  # 1MB
        chunks = 10
        content = b"y" * chunk_size

        with test_file.open("wb") as f:
            for _ in range(chunks):
                f.write(content)

        result = hash_file_blake3(test_file)

        # Verify against streamed hash
        hasher = blake3.blake3()
        with test_file.open("rb") as f:
            for chunk in iter(lambda: f.read(8192 * 1024), b""):
                hasher.update(chunk)
        expected = hasher.hexdigest()

        assert result == expected

    def test_hash_empty_file(self, tmp_path: Path):
        """Test hashing an empty file with BLAKE3."""
        try:
            import blake3
        except ImportError:  # noqa: no-except-logging
            pytest.skip("blake3 not available")

        test_file = tmp_path / "empty.txt"
        test_file.touch()

        result = hash_file_blake3(test_file)
        expected = blake3.blake3(b"").hexdigest()
        assert result == expected

    def test_blake3_not_available(self, tmp_path: Path, monkeypatch):
        """Test graceful handling when blake3 is not available."""
        test_file = tmp_path / "test.txt"
        test_file.write_bytes(b"test")

        # Mock blake3 import to fail
        import sys

        monkeypatch.setitem(sys.modules, "blake3", None)

        result = hash_file_blake3(test_file)
        assert result == ""


@pytest.mark.unit
class TestComputeDualHash:
    """Tests for compute_dual_hash convenience function."""

    def test_compute_both_hashes(self, tmp_path: Path):
        """Test computing both SHA256 and BLAKE3."""
        test_file = tmp_path / "test.bin"
        content = b"test content for dual hash"
        test_file.write_bytes(content)

        sha256_result, blake3_result = compute_dual_hash(test_file)

        # Verify SHA256
        expected_sha256 = hashlib.sha256(content).hexdigest()
        assert sha256_result == expected_sha256

        # Verify BLAKE3 if available
        try:
            import blake3

            expected_blake3 = blake3.blake3(content).hexdigest()
            assert blake3_result == expected_blake3
        except ImportError:  # noqa: no-except-logging
            assert blake3_result == ""

    def test_compute_dual_hash_large_file(self, tmp_path: Path):
        """Test dual hashing on large file."""
        test_file = tmp_path / "large.bin"
        # Create 5MB file
        content = b"z" * (5 * 1024 * 1024)
        test_file.write_bytes(content)

        sha256_result, blake3_result = compute_dual_hash(test_file)

        # Both should be 64 char hex strings (or blake3 empty if not available)
        assert len(sha256_result) == 64
        assert blake3_result == "" or len(blake3_result) == 64

    def test_compute_dual_hash_empty_file(self, tmp_path: Path):
        """Test dual hashing on empty file."""
        test_file = tmp_path / "empty.bin"
        test_file.touch()

        sha256_result, blake3_result = compute_dual_hash(test_file)

        expected_sha256 = hashlib.sha256(b"").hexdigest()
        assert sha256_result == expected_sha256

        try:
            import blake3

            expected_blake3 = blake3.blake3(b"").hexdigest()
            assert blake3_result == expected_blake3
        except ImportError:  # noqa: no-except-logging
            assert blake3_result == ""
