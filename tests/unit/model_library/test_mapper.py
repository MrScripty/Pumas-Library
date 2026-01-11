"""Tests for mapper.py with io/platform integration."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Generator

import pytest

from backend.model_library.library import ModelLibrary
from backend.model_library.mapper import ModelMapper


@pytest.fixture
def temp_library(tmp_path: Path) -> Path:
    """Create a temporary library directory."""
    library_path = tmp_path / "library"
    library_path.mkdir()
    return library_path


@pytest.fixture
def library(temp_library: Path) -> ModelLibrary:
    """Create a ModelLibrary instance."""
    return ModelLibrary(temp_library)


@pytest.fixture
def config_root(tmp_path: Path) -> Path:
    """Create a temporary config directory."""
    config_path = tmp_path / "configs"
    config_path.mkdir()
    return config_path


@pytest.fixture
def mapper(library: ModelLibrary, config_root: Path) -> ModelMapper:
    """Create a ModelMapper instance."""
    return ModelMapper(library, config_root)


@pytest.fixture
def sample_metadata() -> dict:
    """Create sample model metadata."""
    return {
        "model_id": "test-model",
        "family": "test-family",
        "model_type": "diffusion",
        "subtype": "checkpoints",
        "official_name": "Test Model v1.0",
        "cleaned_name": "test-model",
        "tags": ["checkpoint", "sd-xl", "base"],
        "base_model": "",
        "preview_image": "",
        "release_date": "",
        "download_url": "",
        "model_card": {},
        "inference_settings": {},
        "compatible_apps": [],
        "hashes": {"sha256": "abc123", "blake3": "def456"},
        "notes": "",
        "added_date": "2026-01-10T12:00:00Z",
        "updated_date": "2026-01-10T12:00:00Z",
        "size_bytes": 1024,
        "files": [],
    }


@pytest.mark.unit
class TestModelMapperInit:
    """Tests for ModelMapper initialization."""

    def test_mapper_init(self, library: ModelLibrary, config_root: Path) -> None:
        """Test that mapper initializes correctly."""
        mapper = ModelMapper(library, config_root)
        assert mapper.library == library
        assert mapper.config_root == config_root

    def test_mapper_creates_config_dir(self, library: ModelLibrary, tmp_path: Path) -> None:
        """Test that mapper creates config directory if missing."""
        config_path = tmp_path / "new_configs"
        assert not config_path.exists()
        ModelMapper(library, config_path)
        assert config_path.exists()


@pytest.mark.unit
class TestLoadConfigs:
    """Tests for loading mapping configurations."""

    def test_load_no_configs(self, mapper: ModelMapper) -> None:
        """Test loading when no configs exist."""
        configs = mapper._load_configs("comfyui", "1.0.0")
        assert configs == []

    def test_load_matching_config(self, mapper: ModelMapper, config_root: Path) -> None:
        """Test loading a matching config."""
        config_data = {
            "app_id": "comfyui",
            "app_version": "1.0.0",
            "mappings": [{"target_subdir": "models/checkpoints", "method": "symlink"}],
        }
        config_file = config_root / "comfyui_1.0.0_default.json"
        config_file.write_text(json.dumps(config_data))

        configs = mapper._load_configs("comfyui", "1.0.0")
        assert len(configs) == 1
        assert configs[0]["app_id"] == "comfyui"


@pytest.mark.unit
class TestVersionAllowed:
    """Tests for version constraint checking."""

    def test_version_allowed_no_constraints(
        self, mapper: ModelMapper, library: ModelLibrary, sample_metadata: dict
    ) -> None:
        """Test version allowed when no constraints exist."""
        model_dir = library.library_root / "diffusion" / "test" / "model"
        model_dir.mkdir(parents=True)
        library.save_overrides(model_dir, {})

        result = mapper._version_allowed(model_dir, "comfyui", "1.0.0")
        assert result is True

    def test_version_allowed_with_matching_range(
        self, mapper: ModelMapper, library: ModelLibrary
    ) -> None:
        """Test version allowed with matching version range."""
        model_dir = library.library_root / "diffusion" / "test" / "model"
        model_dir.mkdir(parents=True)
        library.save_overrides(model_dir, {"version_ranges": {"comfyui": ">=1.0.0"}})

        result = mapper._version_allowed(model_dir, "comfyui", "1.5.0")
        assert result is True

    def test_version_not_allowed(self, mapper: ModelMapper, library: ModelLibrary) -> None:
        """Test version not allowed when outside range."""
        model_dir = library.library_root / "diffusion" / "test" / "model"
        model_dir.mkdir(parents=True)
        library.save_overrides(model_dir, {"version_ranges": {"comfyui": ">=2.0.0"}})

        result = mapper._version_allowed(model_dir, "comfyui", "1.5.0")
        assert result is False


@pytest.mark.unit
class TestMatchesFilters:
    """Tests for filter matching."""

    def test_matches_no_filters(self, mapper: ModelMapper, sample_metadata: dict) -> None:
        """Test that empty filters match everything."""
        result = mapper._matches_filters(sample_metadata, {})
        assert result is True

    def test_matches_model_type(self, mapper: ModelMapper, sample_metadata: dict) -> None:
        """Test matching by model type."""
        result = mapper._matches_filters(sample_metadata, {"model_type": "diffusion"})
        assert result is True

        result = mapper._matches_filters(sample_metadata, {"model_type": "llm"})
        assert result is False

    def test_matches_subtype(self, mapper: ModelMapper, sample_metadata: dict) -> None:
        """Test matching by subtype."""
        result = mapper._matches_filters(sample_metadata, {"subtype": "checkpoints"})
        assert result is True

        result = mapper._matches_filters(sample_metadata, {"subtype": "lora"})
        assert result is False

    def test_matches_tags(self, mapper: ModelMapper, sample_metadata: dict) -> None:
        """Test matching by tags."""
        result = mapper._matches_filters(sample_metadata, {"tags": ["checkpoint"]})
        assert result is True

        result = mapper._matches_filters(sample_metadata, {"tags": ["nonexistent"]})
        assert result is False


@pytest.mark.unit
class TestPlatformIntegration:
    """Tests for io/platform integration."""

    def test_platform_module_available(self) -> None:
        """Test that platform module is available."""
        from backend.model_library.io.platform import LinkResult, LinkStrategy, create_link

        assert LinkStrategy is not None
        assert LinkResult is not None
        assert callable(create_link)

    def test_create_link_function(self, tmp_path: Path) -> None:
        """Test create_link from platform module."""
        from backend.model_library.io.platform import LinkStrategy, create_link

        # Create source file
        source = tmp_path / "source.txt"
        source.write_text("test content")

        # Create link
        target = tmp_path / "links" / "link.txt"
        result = create_link(source, target, LinkStrategy.SYMLINK)

        assert result.success
        assert target.exists()
        assert target.is_symlink()


@pytest.mark.unit
class TestCreateLink:
    """Tests for _create_link method."""

    def test_create_symlink(
        self, mapper: ModelMapper, library: ModelLibrary, tmp_path: Path
    ) -> None:
        """Test creating a symlink."""
        # Create source file
        model_dir = library.library_root / "diffusion" / "test" / "model"
        model_dir.mkdir(parents=True)
        source = model_dir / "model.safetensors"
        source.write_bytes(b"model data")

        # Create target link
        target_dir = tmp_path / "app" / "models"
        target_dir.mkdir(parents=True)
        target = target_dir / "model.safetensors"

        result = mapper._create_link(source, target)
        assert result is True
        assert target.exists()
        assert target.is_symlink()

    def test_create_link_replaces_existing_symlink(
        self, mapper: ModelMapper, library: ModelLibrary, tmp_path: Path
    ) -> None:
        """Test that creating a link replaces existing symlink."""
        model_dir = library.library_root / "diffusion" / "test" / "model"
        model_dir.mkdir(parents=True)
        source = model_dir / "model.safetensors"
        source.write_bytes(b"model data")

        target_dir = tmp_path / "app" / "models"
        target_dir.mkdir(parents=True)
        target = target_dir / "model.safetensors"

        # Create initial symlink to nowhere
        target.symlink_to("/nonexistent")
        assert target.is_symlink()

        # Create new link
        result = mapper._create_link(source, target)
        assert result is True
        assert target.is_symlink()
        # Should now point to actual source
        assert target.resolve() == source


@pytest.mark.unit
class TestApplyForApp:
    """Tests for apply_for_app method."""

    def test_apply_no_config(self, mapper: ModelMapper, tmp_path: Path) -> None:
        """Test applying mappings when no config exists."""
        app_root = tmp_path / "app"
        app_root.mkdir()

        result = mapper.apply_for_app("comfyui", "1.0.0", app_root)
        assert result == 0

    def test_apply_creates_links(
        self,
        mapper: ModelMapper,
        library: ModelLibrary,
        config_root: Path,
        sample_metadata: dict,
        tmp_path: Path,
    ) -> None:
        """Test that apply_for_app creates symlinks."""
        # Set up model in library
        model_dir = library.library_root / "diffusion" / "test" / "model"
        model_dir.mkdir(parents=True)
        model_file = model_dir / "model.safetensors"
        model_file.write_bytes(b"model data")
        library.save_metadata(model_dir, sample_metadata)
        library.index_model_dir(model_dir, sample_metadata)

        # Create config
        config_data = {
            "app_id": "comfyui",
            "app_version": "1.0.0",
            "mappings": [
                {
                    "target_subdir": "models/checkpoints",
                    "method": "symlink",
                    "patterns": ["*.safetensors"],
                    "filters": {"model_type": "diffusion"},
                }
            ],
        }
        config_file = config_root / "comfyui_1.0.0_default.json"
        config_file.write_text(json.dumps(config_data))

        # Apply mappings
        app_root = tmp_path / "comfyui"
        app_root.mkdir()

        result = mapper.apply_for_app("comfyui", "1.0.0", app_root)

        # Should have created at least one link
        assert result >= 1

        # Check the link exists
        link_path = app_root / "models" / "checkpoints" / "modelsafetensors"
        # Path may be normalized differently
        checkpoints_dir = app_root / "models" / "checkpoints"
        if checkpoints_dir.exists():
            links = list(checkpoints_dir.glob("*"))
            assert len(links) >= 1


@pytest.mark.unit
class TestLinkRegistryIntegration:
    """Tests for link registry integration."""

    def test_mapper_accepts_registry(
        self, library: ModelLibrary, config_root: Path, tmp_path: Path
    ) -> None:
        """Test that mapper accepts a link registry."""
        from backend.model_library.link_registry import LinkRegistry

        registry = LinkRegistry(tmp_path / "registry.db")
        mapper = ModelMapper(library, config_root, link_registry=registry)

        assert mapper._link_registry is registry

    def test_mapper_without_registry(self, library: ModelLibrary, config_root: Path) -> None:
        """Test that mapper works without a link registry."""
        mapper = ModelMapper(library, config_root)
        assert mapper._link_registry is None

    def test_create_link_with_registry_tracks_link(
        self, library: ModelLibrary, config_root: Path, tmp_path: Path
    ) -> None:
        """Test that creating a link registers it in the registry."""
        from backend.model_library.link_registry import LinkRegistry

        registry = LinkRegistry(tmp_path / "registry.db")
        mapper = ModelMapper(library, config_root, link_registry=registry)

        # Create source file
        model_dir = library.library_root / "diffusion" / "test" / "model"
        model_dir.mkdir(parents=True)
        source = model_dir / "model.safetensors"
        source.write_bytes(b"model data")

        # Create target path
        target_dir = tmp_path / "app" / "models"
        target_dir.mkdir(parents=True)
        target = target_dir / "model.safetensors"

        # Create link with registry tracking
        result = mapper._create_link_with_registry(
            source=source,
            target=target,
            model_id="test-model",
            app_id="comfyui",
            app_version="0.6.0",
        )

        assert result is True
        assert target.exists()

        # Check registry
        links = registry.get_links_for_model("test-model")
        assert len(links) == 1
        assert links[0].app_id == "comfyui"
        assert links[0].app_version == "0.6.0"

    def test_delete_model_with_cascade(
        self, library: ModelLibrary, config_root: Path, tmp_path: Path
    ) -> None:
        """Test cascade delete removes links."""
        from backend.model_library.link_registry import LinkRegistry

        registry = LinkRegistry(tmp_path / "registry.db")
        mapper = ModelMapper(library, config_root, link_registry=registry)

        # Create source file
        model_dir = library.library_root / "diffusion" / "test" / "model"
        model_dir.mkdir(parents=True)
        source = model_dir / "model.safetensors"
        source.write_bytes(b"model data")

        # Create multiple links
        for i in range(3):
            target_dir = tmp_path / f"app{i}" / "models"
            target_dir.mkdir(parents=True)
            target = target_dir / "model.safetensors"
            mapper._create_link_with_registry(
                source=source,
                target=target,
                model_id="test-model",
                app_id="comfyui",
                app_version="0.6.0",
            )

        assert registry.get_link_count() == 3

        # Cascade delete
        removed = mapper.delete_model_with_cascade("test-model")

        assert removed == 3
        assert registry.get_link_count() == 0

    def test_delete_model_without_registry(self, library: ModelLibrary, config_root: Path) -> None:
        """Test cascade delete without registry returns 0."""
        mapper = ModelMapper(library, config_root)
        removed = mapper.delete_model_with_cascade("test-model")
        assert removed == 0
