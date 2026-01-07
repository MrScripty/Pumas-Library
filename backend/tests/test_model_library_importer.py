"""Tests for the model library importer."""

from pathlib import Path

from backend.model_library.importer import ModelImporter
from backend.model_library.library import ModelLibrary


def test_import_file_moves_and_writes_metadata(tmp_path: Path):
    library = ModelLibrary(tmp_path / "models")
    importer = ModelImporter(library)

    source_file = tmp_path / "My Model.safetensors"
    source_file.write_text("data")

    model_dir = importer.import_path(source_file, "family", "My Model")

    assert not source_file.exists()
    metadata_path = model_dir / "metadata.json"
    assert metadata_path.exists()

    metadata = library.load_metadata(model_dir)
    assert metadata is not None
    assert metadata["official_name"] == "My Model"
    assert metadata["cleaned_name"] == model_dir.name
    assert metadata["files"][0]["original_name"] == "My Model.safetensors"


def test_import_directory_moves_files_and_removes_empty_dir(tmp_path: Path):
    library = ModelLibrary(tmp_path / "models")
    importer = ModelImporter(library)

    source_dir = tmp_path / "source"
    source_dir.mkdir()
    (source_dir / "model.bin").write_text("data")

    model_dir = importer.import_path(source_dir, "family", "Model Bin")

    assert not source_dir.exists()
    assert (model_dir / "model.bin").exists()
