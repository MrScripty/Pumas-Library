"""Test search package initialization."""

from __future__ import annotations

import pytest


@pytest.mark.unit
def test_search_package_imports():
    """Test that search package can be imported."""
    import backend.model_library.search

    assert hasattr(backend.model_library.search, "__all__")
    assert isinstance(backend.model_library.search.__all__, list)
