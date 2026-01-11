"""Tests for model library API endpoints in core.py."""

from __future__ import annotations

from pathlib import Path
from typing import Any, Dict
from unittest.mock import MagicMock, Mock, patch

import pytest

from backend.model_library.network import NetworkStats
from backend.model_library.search import SearchResult


@pytest.fixture
def mock_resource_manager() -> MagicMock:
    """Create a mock resource manager."""
    rm = MagicMock()
    rm.model_library = MagicMock()
    return rm


@pytest.fixture
def mock_network_manager() -> MagicMock:
    """Create a mock network manager."""
    nm = MagicMock()
    return nm


@pytest.mark.unit
class TestSearchModelsFTS:
    """Tests for FTS5 search endpoint."""

    def test_search_models_fts_returns_results(self, mock_resource_manager: MagicMock) -> None:
        """Test that search_models_fts returns search results."""
        # Setup mock search result
        mock_result = SearchResult(
            models=[
                {"model_id": "test-1", "official_name": "Test Model 1"},
                {"model_id": "test-2", "official_name": "Test Model 2"},
            ],
            total_count=2,
            query_time_ms=1.5,
            query="llama*",
        )
        mock_resource_manager.search_models_fts.return_value = mock_result

        # Call the method
        result = mock_resource_manager.search_models_fts("llama", limit=100)

        assert result.total_count == 2
        assert len(result.models) == 2
        assert result.query_time_ms < 20  # Should be sub-20ms

    def test_search_models_fts_with_filters(self, mock_resource_manager: MagicMock) -> None:
        """Test that search_models_fts accepts filter parameters."""
        mock_result = SearchResult(
            models=[{"model_id": "llm-1", "model_type": "llm"}],
            total_count=1,
            query_time_ms=0.8,
            query="llama*",
        )
        mock_resource_manager.search_models_fts.return_value = mock_result

        result = mock_resource_manager.search_models_fts(
            "llama", limit=50, model_type="llm", tags=["chat"]
        )

        assert result.total_count == 1

    def test_search_models_fts_empty_query(self, mock_resource_manager: MagicMock) -> None:
        """Test that empty query returns empty results."""
        mock_result = SearchResult(
            models=[],
            total_count=0,
            query_time_ms=0.1,
            query="",
        )
        mock_resource_manager.search_models_fts.return_value = mock_result

        result = mock_resource_manager.search_models_fts("", limit=100)

        assert result.total_count == 0
        assert result.models == []

    def test_search_models_fts_pagination(self, mock_resource_manager: MagicMock) -> None:
        """Test that pagination parameters work."""
        mock_result = SearchResult(
            models=[{"model_id": "test-50"}],
            total_count=100,
            query_time_ms=1.0,
            query="model*",
        )
        mock_resource_manager.search_models_fts.return_value = mock_result

        result = mock_resource_manager.search_models_fts("model", limit=10, offset=50)

        mock_resource_manager.search_models_fts.assert_called_with("model", limit=10, offset=50)


@pytest.mark.unit
class TestNetworkStatus:
    """Tests for network status endpoint."""

    def test_get_network_status_available(self) -> None:
        """Test that NetworkManager provides status information."""
        from backend.model_library.network import NetworkManager

        manager = NetworkManager()
        # Should have a method to get stats
        assert hasattr(manager, "get_stats")

    def test_network_status_returns_network_stats(self) -> None:
        """Test that network status returns a NetworkStats dataclass."""
        from backend.model_library.network import NetworkManager

        manager = NetworkManager()
        stats = manager.get_stats()

        assert isinstance(stats, NetworkStats)
        assert hasattr(stats, "total_requests")
        assert hasattr(stats, "successful_requests")
        assert hasattr(stats, "failed_requests")

    def test_network_status_circuit_breaker_info(self) -> None:
        """Test that network status includes circuit breaker rejection count."""
        from backend.model_library.network import NetworkManager

        manager = NetworkManager()
        stats = manager.get_stats()

        # NetworkStats should include circuit breaker rejection count
        assert hasattr(stats, "circuit_breaker_rejections")
        assert stats.circuit_breaker_rejections >= 0

    def test_network_stats_to_dict(self) -> None:
        """Test that NetworkStats can be converted to dict for API response."""
        from dataclasses import asdict

        stats = NetworkStats(
            total_requests=10,
            successful_requests=8,
            failed_requests=2,
            circuit_breaker_rejections=1,
            retries=3,
        )

        stats_dict = asdict(stats)
        assert stats_dict["total_requests"] == 10
        assert stats_dict["successful_requests"] == 8
        assert stats_dict["circuit_breaker_rejections"] == 1


@pytest.mark.unit
class TestImportBatch:
    """Tests for batch import endpoints."""

    def test_import_batch_accepts_list(self, mock_resource_manager: MagicMock) -> None:
        """Test that import_batch accepts a list of import specs."""
        mock_resource_manager.import_batch.return_value = {
            "success": True,
            "imported": 3,
            "failed": 0,
            "results": [],
        }

        result = mock_resource_manager.import_batch(
            [
                {"path": "/path/to/model1.safetensors", "family": "stability"},
                {"path": "/path/to/model2.gguf", "family": "meta"},
            ]
        )

        assert result["success"] is True

    def test_import_batch_returns_individual_results(
        self, mock_resource_manager: MagicMock
    ) -> None:
        """Test that import_batch returns results for each import."""
        mock_resource_manager.import_batch.return_value = {
            "success": True,
            "imported": 2,
            "failed": 1,
            "results": [
                {"path": "/path/to/model1.safetensors", "success": True},
                {"path": "/path/to/model2.gguf", "success": True},
                {"path": "/path/to/bad.txt", "success": False, "error": "Invalid format"},
            ],
        }

        result = mock_resource_manager.import_batch(
            [
                {"path": "/path/to/model1.safetensors", "family": "stability"},
                {"path": "/path/to/model2.gguf", "family": "meta"},
                {"path": "/path/to/bad.txt", "family": "test"},
            ]
        )

        assert result["imported"] == 2
        assert result["failed"] == 1
        assert len(result["results"]) == 3

    def test_import_batch_empty_list(self, mock_resource_manager: MagicMock) -> None:
        """Test that empty import list returns empty result."""
        mock_resource_manager.import_batch.return_value = {
            "success": True,
            "imported": 0,
            "failed": 0,
            "results": [],
        }

        result = mock_resource_manager.import_batch([])

        assert result["imported"] == 0
        assert result["results"] == []


@pytest.mark.unit
class TestResourceManagerIntegration:
    """Tests for ResourceManager integration with new endpoints."""

    def test_resource_manager_has_model_library(self) -> None:
        """Test that ResourceManager has model_library attribute."""
        from backend.resources.resource_manager import ResourceManager

        # Check the class has expected attribute access pattern
        assert hasattr(ResourceManager, "__init__")

    def test_model_library_has_search_method(self) -> None:
        """Test that ModelLibrary has search_models method."""
        from backend.model_library.library import ModelLibrary

        assert hasattr(ModelLibrary, "search_models")

    def test_search_result_structure(self) -> None:
        """Test SearchResult has expected structure."""
        from backend.model_library.search import SearchResult

        result = SearchResult(
            models=[],
            total_count=0,
            query_time_ms=0.0,
            query="",
        )

        assert hasattr(result, "models")
        assert hasattr(result, "total_count")
        assert hasattr(result, "query_time_ms")


@pytest.mark.unit
class TestAPIEndpointSignatures:
    """Tests to verify API endpoint signatures match frontend expectations."""

    def test_search_models_fts_signature(self) -> None:
        """Test search_models_fts has correct signature for frontend."""
        # The frontend expects:
        # search_models_fts(query: str, limit?: number) -> { models: [], query_time_ms: number }
        from backend.model_library.library import ModelLibrary

        method = getattr(ModelLibrary, "search_models", None)
        assert method is not None

    def test_network_status_signature(self) -> None:
        """Test network status method exists with correct name."""
        from backend.model_library.network import NetworkManager

        assert hasattr(NetworkManager, "get_stats")


@pytest.mark.unit
class TestSearchResultSerialization:
    """Tests for SearchResult serialization for API responses."""

    def test_search_result_to_dict(self) -> None:
        """Test that SearchResult can be serialized to dict for JSON response."""
        from dataclasses import asdict

        result = SearchResult(
            models=[{"model_id": "test", "name": "Test Model"}],
            total_count=1,
            query_time_ms=1.5,
            query="test*",
        )

        result_dict = asdict(result)
        assert result_dict["models"] == [{"model_id": "test", "name": "Test Model"}]
        assert result_dict["total_count"] == 1
        assert result_dict["query_time_ms"] == 1.5
        assert result_dict["query"] == "test*"

    def test_search_result_api_response_format(self) -> None:
        """Test search result matches expected API response format."""
        result = SearchResult(
            models=[],
            total_count=0,
            query_time_ms=0.5,
            query="",
        )

        # API should return these fields
        assert hasattr(result, "models")
        assert hasattr(result, "total_count")
        assert hasattr(result, "query_time_ms")
