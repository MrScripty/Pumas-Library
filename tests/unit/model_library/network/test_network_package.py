"""Test network package initialization."""

from __future__ import annotations

import pytest


@pytest.mark.unit
def test_network_package_imports():
    """Test that network package can be imported."""
    import backend.model_library.network

    assert hasattr(backend.model_library.network, "__all__")
    assert isinstance(backend.model_library.network.__all__, list)
