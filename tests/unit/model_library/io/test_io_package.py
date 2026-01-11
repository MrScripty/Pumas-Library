"""Test io package initialization."""

from __future__ import annotations

import pytest


@pytest.mark.unit
def test_io_package_imports():
    """Test that io package can be imported."""
    import backend.model_library.io

    assert hasattr(backend.model_library.io, "__all__")
    assert isinstance(backend.model_library.io.__all__, list)
