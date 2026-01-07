"""Tests for the model library downloader."""

from __future__ import annotations

import sys
from datetime import datetime
from pathlib import Path

import pytest

from backend.model_library.downloader import ModelDownloader
from backend.model_library.library import ModelLibrary


class _FakeCard:
    def to_dict(self):
        return {"base_model": "sdxl"}

    def get(self, key, default=""):
        return {"base_model": "sdxl"}.get(key, default)


class _FakeSibling:
    def __init__(self, rfilename: str) -> None:
        self.rfilename = rfilename


class _FakeInfo:
    def __init__(self) -> None:
        self.last_modified = datetime(2024, 1, 1)
        self.tags = ["tag"]
        self.card_data = _FakeCard()
        self.siblings = [_FakeSibling("preview.jpg")]


class _FakeApi:
    def model_info(self, repo_id: str):
        return _FakeInfo()


class _FakeHubModule:
    def __init__(self):
        self._api = _FakeApi()

    def login(self, token: str) -> None:
        return None

    class HfApi:
        def __init__(self):
            self._inner = _FakeApi()

        def model_info(self, repo_id: str):
            return self._inner.model_info(repo_id)

    def snapshot_download(
        self, repo_id: str, local_dir: Path, local_dir_use_symlinks: bool, ignore_patterns
    ):
        target = Path(local_dir) / "model.safetensors"
        target.write_text("data")
        return str(local_dir)

    def hf_hub_download(self, repo_id: str, filename: str, local_dir: Path):
        target = Path(local_dir) / filename
        target.write_text("image")
        return str(target)


@pytest.fixture
def fake_hub(monkeypatch):
    module = _FakeHubModule()
    monkeypatch.setitem(sys.modules, "huggingface_hub", module)
    return module


def test_download_from_hf_creates_metadata(tmp_path: Path, fake_hub):
    library = ModelLibrary(tmp_path / "models")
    downloader = ModelDownloader(library)

    model_dir = downloader.download_from_hf(
        repo_id="org/model",
        family="family",
        official_name="My Model",
        model_type="diffusion",
        subtype="checkpoints",
    )

    metadata = library.load_metadata(model_dir)
    assert metadata is not None
    assert metadata["official_name"] == "My Model"
    assert metadata["base_model"] == "sdxl"
    assert metadata["preview_image"] == "preview.png"
    assert metadata["files"][0]["original_name"] == "model.safetensors"
