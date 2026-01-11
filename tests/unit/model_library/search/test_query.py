"""Tests for search query builder."""

from __future__ import annotations

import sqlite3
import time
from typing import Any

import pytest

from backend.model_library.search.fts5 import (
    FTS5Manager,
    create_fts5_table,
    populate_fts5_from_models,
)
from backend.model_library.search.query import (
    SearchQuery,
    SearchResult,
    build_fts5_query,
    escape_fts5_term,
    search_models,
)


@pytest.fixture
def temp_db() -> sqlite3.Connection:
    """Create a temporary in-memory database with models table."""
    conn = sqlite3.connect(":memory:")
    conn.row_factory = sqlite3.Row
    conn.execute(
        """
        CREATE TABLE models (
            id TEXT PRIMARY KEY,
            path TEXT NOT NULL,
            cleaned_name TEXT NOT NULL,
            official_name TEXT NOT NULL,
            model_type TEXT NOT NULL,
            tags_json TEXT NOT NULL,
            hashes_json TEXT NOT NULL,
            metadata_json TEXT NOT NULL,
            updated_at TEXT NOT NULL
        )
    """
    )
    conn.commit()
    return conn


@pytest.fixture
def search_db(temp_db: sqlite3.Connection) -> sqlite3.Connection:
    """Create a database with FTS5 and sample data for search tests."""
    models = [
        {
            "id": "diffusion/stability/sdxl-base",
            "path": "diffusion/stability/sdxl-base",
            "cleaned_name": "sdxl-base",
            "official_name": "SDXL Base 1.0",
            "model_type": "diffusion",
            "tags_json": '["checkpoint", "sd-xl", "base"]',
            "hashes_json": '{"blake3": "abc123"}',
            "metadata_json": '{"family": "stability", "description": "Stable Diffusion XL base model"}',
            "updated_at": "2026-01-10T12:00:00Z",
        },
        {
            "id": "llm/meta/llama-3-8b",
            "path": "llm/meta/llama-3-8b",
            "cleaned_name": "llama-3-8b",
            "official_name": "Llama 3 8B Instruct",
            "model_type": "llm",
            "tags_json": '["gguf", "instruct", "chat"]',
            "hashes_json": '{"blake3": "def456"}',
            "metadata_json": '{"family": "meta", "description": "Meta Llama 3 8B parameter chat model"}',
            "updated_at": "2026-01-10T12:00:00Z",
        },
        {
            "id": "diffusion/runwayml/sd-v1-5",
            "path": "diffusion/runwayml/sd-v1-5",
            "cleaned_name": "sd-v1-5",
            "official_name": "Stable Diffusion v1.5",
            "model_type": "diffusion",
            "tags_json": '["checkpoint", "sd-1.5"]',
            "hashes_json": '{"blake3": "ghi789"}',
            "metadata_json": '{"family": "runwayml", "description": "Classic SD 1.5 model"}',
            "updated_at": "2026-01-10T12:00:00Z",
        },
        {
            "id": "lora/civitai/detail-enhancer",
            "path": "lora/civitai/detail-enhancer",
            "cleaned_name": "detail-enhancer",
            "official_name": "Detail Enhancer LoRA",
            "model_type": "lora",
            "tags_json": '["lora", "detail", "enhancement"]',
            "hashes_json": '{"blake3": "jkl012"}',
            "metadata_json": '{"family": "civitai", "description": "Adds fine details to images"}',
            "updated_at": "2026-01-10T12:00:00Z",
        },
        {
            "id": "llm/mistral/mistral-7b",
            "path": "llm/mistral/mistral-7b",
            "cleaned_name": "mistral-7b",
            "official_name": "Mistral 7B v0.2",
            "model_type": "llm",
            "tags_json": '["gguf", "base"]',
            "hashes_json": '{"blake3": "mno345"}',
            "metadata_json": '{"family": "mistral", "description": "Mistral AI 7B base model"}',
            "updated_at": "2026-01-10T12:00:00Z",
        },
    ]

    for model in models:
        temp_db.execute(
            """
            INSERT INTO models (
                id, path, cleaned_name, official_name, model_type,
                tags_json, hashes_json, metadata_json, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            """,
            (
                model["id"],
                model["path"],
                model["cleaned_name"],
                model["official_name"],
                model["model_type"],
                model["tags_json"],
                model["hashes_json"],
                model["metadata_json"],
                model["updated_at"],
            ),
        )
    temp_db.commit()

    # Set up FTS5
    FTS5Manager(temp_db)

    return temp_db


@pytest.mark.unit
class TestEscapeFTS5Term:
    """Tests for escape_fts5_term function."""

    def test_escape_simple_term(self):
        """Test that simple terms pass through unchanged."""
        assert escape_fts5_term("hello") == "hello"
        assert escape_fts5_term("world") == "world"

    def test_escape_hyphenated_term(self):
        """Test escaping hyphenated terms."""
        assert escape_fts5_term("sd-xl") == '"sd-xl"'
        assert escape_fts5_term("llama-3-8b") == '"llama-3-8b"'

    def test_escape_dotted_term(self):
        """Test escaping terms with dots."""
        assert escape_fts5_term("v1.5") == '"v1.5"'
        assert escape_fts5_term("model.safetensors") == '"model.safetensors"'

    def test_escape_term_with_quotes(self):
        """Test escaping terms that already have quotes."""
        # Terms without special chars pass through unchanged
        assert escape_fts5_term('test"value') == 'test"value'
        # But if term has special chars AND quotes, quotes are escaped
        assert escape_fts5_term('test-"value') == '"test-""value"'

    def test_empty_term(self):
        """Test handling of empty terms."""
        assert escape_fts5_term("") == ""

    def test_whitespace_term(self):
        """Test handling of whitespace-only terms."""
        assert escape_fts5_term("   ") == ""


@pytest.mark.unit
class TestBuildFTS5Query:
    """Tests for build_fts5_query function."""

    def test_single_word_query(self):
        """Test building query from single word."""
        query = build_fts5_query("llama")
        assert "llama*" in query

    def test_multi_word_query(self):
        """Test building query from multiple words."""
        query = build_fts5_query("stable diffusion")
        assert "stable*" in query
        assert "diffusion*" in query
        assert " OR " in query

    def test_hyphenated_query(self):
        """Test building query with hyphenated terms."""
        query = build_fts5_query("sd-xl")
        # Hyphenated term should be quoted
        assert '"sd-xl"*' in query

    def test_empty_query(self):
        """Test building query from empty string."""
        query = build_fts5_query("")
        assert query == ""

    def test_prefix_matching(self):
        """Test that prefix matching is enabled."""
        query = build_fts5_query("llam")
        assert "llam*" in query  # Should match "llama"

    def test_case_insensitive(self):
        """Test that query terms are lowercased."""
        query = build_fts5_query("LLAMA")
        assert "llama*" in query

    def test_special_characters_stripped(self):
        """Test that special FTS5 characters are handled."""
        query = build_fts5_query("test:model")
        # Colon is special in FTS5, should be handled
        assert "test" in query or "model" in query


@pytest.mark.unit
class TestSearchQuery:
    """Tests for SearchQuery dataclass."""

    def test_default_query(self):
        """Test default query configuration."""
        query = SearchQuery(terms="test")
        assert query.terms == "test"
        assert query.limit == 100
        assert query.model_type is None
        assert query.tags is None

    def test_custom_query(self):
        """Test custom query configuration."""
        query = SearchQuery(
            terms="llama",
            limit=50,
            model_type="llm",
            tags=["gguf", "instruct"],
        )
        assert query.terms == "llama"
        assert query.limit == 50
        assert query.model_type == "llm"
        assert query.tags == ["gguf", "instruct"]


@pytest.mark.unit
class TestSearchResult:
    """Tests for SearchResult dataclass."""

    def test_search_result_creation(self):
        """Test creating a search result."""
        result = SearchResult(
            models=[{"id": "test", "name": "Test Model"}],
            total_count=1,
            query_time_ms=5.2,
            query="test*",
        )
        assert len(result.models) == 1
        assert result.total_count == 1
        assert result.query_time_ms == 5.2
        assert result.query == "test*"


@pytest.mark.unit
class TestSearchModels:
    """Tests for search_models function."""

    def test_search_by_name(self, search_db: sqlite3.Connection):
        """Test searching models by name."""
        result = search_models(search_db, "llama")
        assert result.total_count >= 1
        assert any("llama" in m["official_name"].lower() for m in result.models)

    def test_search_by_type(self, search_db: sqlite3.Connection):
        """Test searching models by type."""
        result = search_models(search_db, "diffusion")
        assert result.total_count >= 1
        assert all(m["model_type"] == "diffusion" for m in result.models)

    def test_search_by_family(self, search_db: sqlite3.Connection):
        """Test searching models by family."""
        result = search_models(search_db, "stability")
        assert result.total_count >= 1
        assert any("stability" in str(m.get("family", "")).lower() for m in result.models)

    def test_search_by_tag(self, search_db: sqlite3.Connection):
        """Test searching models by tag."""
        result = search_models(search_db, "gguf")
        assert result.total_count >= 1
        # All results should have gguf tag
        for model in result.models:
            assert "gguf" in str(model.get("tags", [])).lower()

    def test_search_prefix_matching(self, search_db: sqlite3.Connection):
        """Test that prefix matching works."""
        result = search_models(search_db, "llam")
        assert result.total_count >= 1
        assert any("llama" in m["official_name"].lower() for m in result.models)

    def test_search_case_insensitive(self, search_db: sqlite3.Connection):
        """Test case-insensitive search."""
        result_lower = search_models(search_db, "llama")
        result_upper = search_models(search_db, "LLAMA")
        assert result_lower.total_count == result_upper.total_count

    def test_search_no_results(self, search_db: sqlite3.Connection):
        """Test search with no matching results."""
        result = search_models(search_db, "nonexistentmodel12345")
        assert result.total_count == 0
        assert len(result.models) == 0

    def test_search_empty_query(self, search_db: sqlite3.Connection):
        """Test search with empty query returns all models."""
        result = search_models(search_db, "")
        # Empty query should return all models
        assert result.total_count == 5

    def test_search_limit(self, search_db: sqlite3.Connection):
        """Test search respects limit parameter."""
        result = search_models(search_db, "", limit=2)
        assert len(result.models) == 2

    def test_search_returns_metadata(self, search_db: sqlite3.Connection):
        """Test that search returns full metadata."""
        result = search_models(search_db, "sdxl")
        assert result.total_count >= 1
        model = result.models[0]
        assert "id" in model
        assert "official_name" in model
        assert "model_type" in model

    def test_search_performance(self, search_db: sqlite3.Connection):
        """Test that search completes quickly."""
        result = search_models(search_db, "llama")
        # Should complete in under 100ms even for small datasets
        assert result.query_time_ms < 100

    def test_search_with_hyphenated_term(self, search_db: sqlite3.Connection):
        """Test searching with hyphenated terms."""
        result = search_models(search_db, "sd-xl")
        # Should find SDXL model
        assert result.total_count >= 1

    def test_search_multi_term(self, search_db: sqlite3.Connection):
        """Test searching with multiple terms."""
        result = search_models(search_db, "stable diffusion")
        # Should find diffusion models
        assert result.total_count >= 1


@pytest.mark.unit
class TestSearchWithFilters:
    """Tests for search with additional filters."""

    def test_search_filter_by_model_type(self, search_db: sqlite3.Connection):
        """Test filtering search results by model type."""
        query = SearchQuery(terms="", model_type="llm")
        result = search_models(search_db, query.terms, model_type=query.model_type)
        assert all(m["model_type"] == "llm" for m in result.models)

    def test_search_filter_by_multiple_types(self, search_db: sqlite3.Connection):
        """Test filtering by multiple model types."""
        result = search_models(search_db, "", model_type=["diffusion", "lora"])  # type: ignore
        assert all(m["model_type"] in ["diffusion", "lora"] for m in result.models)


@pytest.mark.unit
class TestSearchEdgeCases:
    """Edge case tests for search functionality."""

    def test_search_unicode_terms(self, search_db: sqlite3.Connection):
        """Test searching with unicode characters."""
        # Should not crash
        result = search_models(search_db, "模型")
        assert result.query_time_ms >= 0

    def test_search_very_long_query(self, search_db: sqlite3.Connection):
        """Test searching with very long query."""
        long_query = "test " * 100
        result = search_models(search_db, long_query)
        assert result.query_time_ms >= 0

    def test_search_special_characters(self, search_db: sqlite3.Connection):
        """Test searching with special characters."""
        special_queries = ["test'value", 'test"value', "test*value", "test:value"]
        for query in special_queries:
            # Should not crash
            result = search_models(search_db, query)
            assert result.query_time_ms >= 0
