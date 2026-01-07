"""Tests for model_library package exports."""

from backend import model_library


def test_package_exports():
    assert "ModelLibrary" in model_library.__all__
    assert "ModelMapper" in model_library.__all__
    assert "ModelImporter" in model_library.__all__
    assert "ModelDownloader" in model_library.__all__
