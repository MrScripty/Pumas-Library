"""Test search package initialization."""

from __future__ import annotations

import pytest


@pytest.mark.unit
def test_search_package_imports():
    """Test that search package can be imported."""
    import backend.model_library.search

    assert hasattr(backend.model_library.search, "__all__")
    assert isinstance(backend.model_library.search.__all__, list)


@pytest.mark.unit
def test_fts5_exports():
    """Test that FTS5 components are exported."""
    from backend.model_library.search import (
        FTS5Config,
        FTS5Manager,
        create_fts5_table,
        create_fts5_triggers,
        drop_fts5_triggers,
        fts5_table_exists,
        migrate_to_fts5,
        populate_fts5_from_models,
    )

    assert FTS5Config is not None
    assert FTS5Manager is not None
    assert create_fts5_table is not None
    assert create_fts5_triggers is not None
    assert drop_fts5_triggers is not None
    assert fts5_table_exists is not None
    assert migrate_to_fts5 is not None
    assert populate_fts5_from_models is not None


@pytest.mark.unit
def test_query_exports():
    """Test that query components are exported."""
    from backend.model_library.search import (
        SearchQuery,
        SearchResult,
        build_fts5_query,
        escape_fts5_term,
        search_models,
    )

    assert SearchQuery is not None
    assert SearchResult is not None
    assert build_fts5_query is not None
    assert escape_fts5_term is not None
    assert search_models is not None
