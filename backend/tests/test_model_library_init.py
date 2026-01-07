"""Tests to cover model_library package initialization."""

import importlib
import sys


def test_model_library_init_executes():
    sys.modules.pop("backend.model_library", None)
    module = importlib.import_module("backend.model_library")
    importlib.reload(module)

    assert hasattr(module, "ModelLibrary")
    assert "ModelLibrary" in module.__all__
