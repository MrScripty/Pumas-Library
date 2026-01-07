"""Tests for model library naming utilities."""

from pathlib import Path

from backend.model_library.naming import normalize_filename, normalize_name, unique_path


def test_normalize_name_strips_invalid_chars():
    assert normalize_name("My Model 100%!") == "MyModel100"


def test_normalize_name_fallback_when_empty():
    assert normalize_name("!!!") == "model"


def test_normalize_name_truncates():
    long_name = "a" * 200
    result = normalize_name(long_name)
    assert len(result) == 128


def test_normalize_filename_preserves_extension():
    assert normalize_filename("My Model.safetensors") == "MyModel.safetensors"


def test_unique_path_suffixes_with_extension(tmp_path):
    base = tmp_path / "model.safetensors"
    base.write_text("data")

    candidate = unique_path(base)
    assert candidate.name == "model-2.safetensors"
