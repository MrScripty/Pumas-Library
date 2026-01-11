"""Test hf package initialization."""

from __future__ import annotations

import pytest


@pytest.mark.unit
def test_hf_package_imports():
    """Test that hf package can be imported."""
    import backend.model_library.hf

    assert hasattr(backend.model_library.hf, "__all__")
    assert isinstance(backend.model_library.hf.__all__, list)
