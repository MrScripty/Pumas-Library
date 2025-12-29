"""
Unit tests for MetadataManager functionality.
"""

import json
from pathlib import Path

import pytest

from backend.metadata_manager import MetadataManager


@pytest.mark.unit
class TestMetadataManagerInit:
    """Tests for MetadataManager initialization."""

    def test_init_creates_directories(self, tmp_path):
        """Test that MetadataManager creates required directories on init."""
        launcher_data = tmp_path / "launcher-data"
        manager = MetadataManager(launcher_data)

        assert manager.metadata_dir.exists()
        assert manager.config_dir.exists()
        assert manager.cache_dir.exists()
        assert manager.version_configs_dir.exists()

    def test_init_with_existing_directories(self, temp_metadata_dir):
        """Test that MetadataManager handles existing directories gracefully."""
        # Create directories before initializing
        temp_metadata_dir.mkdir(parents=True, exist_ok=True)

        manager = MetadataManager(temp_metadata_dir)

        # Should not raise errors
        assert manager.launcher_data_dir == temp_metadata_dir

    def test_file_paths_set_correctly(self, temp_metadata_dir):
        """Test that all file paths are set to expected locations."""
        manager = MetadataManager(temp_metadata_dir)

        assert manager.versions_file == manager.metadata_dir / "versions.json"
        assert manager.models_file == manager.metadata_dir / "models.json"
        assert manager.custom_nodes_file == manager.metadata_dir / "custom_nodes.json"
        assert manager.workflows_file == manager.metadata_dir / "workflows.json"
        assert manager.github_cache_file == manager.cache_dir / "github-releases.json"


@pytest.mark.unit
class TestVersionsMetadata:
    """Tests for versions.json operations."""

    def test_load_versions_returns_default_when_missing(self, metadata_manager):
        """Test that load_versions returns default structure when file doesn't exist."""
        versions = metadata_manager.load_versions()

        assert "installed" in versions
        assert "lastSelectedVersion" in versions
        assert "defaultVersion" in versions
        assert versions["installed"] == {}
        assert versions["lastSelectedVersion"] is None

    def test_save_and_load_versions(self, metadata_manager):
        """Test saving and loading versions metadata."""
        test_data = {
            "installed": {
                "v0.5.0": {"path": "comfyui-versions/v0.5.0"},
                "v0.6.0": {"path": "comfyui-versions/v0.6.0"},
            },
            "lastSelectedVersion": "v0.6.0",
            "defaultVersion": "v0.6.0",
        }

        # Save
        success = metadata_manager.save_versions(test_data)
        assert success is True
        assert metadata_manager.versions_file.exists()

        # Load and verify
        loaded = metadata_manager.load_versions()
        assert loaded == test_data

    def test_get_all_version_tags(self, metadata_manager):
        """Test getting list of all installed version tags."""
        test_data = {
            "installed": {
                "v0.5.0": {"path": "comfyui-versions/v0.5.0"},
                "v0.6.0": {"path": "comfyui-versions/v0.6.0"},
                "v0.4.0": {"path": "comfyui-versions/v0.4.0"},
            },
            "lastSelectedVersion": None,
            "defaultVersion": None,
        }
        metadata_manager.save_versions(test_data)

        tags = metadata_manager.get_all_version_tags()
        assert len(tags) == 3
        assert "v0.5.0" in tags
        assert "v0.6.0" in tags
        assert "v0.4.0" in tags

    def test_version_exists(self, metadata_manager):
        """Test checking if a version exists."""
        test_data = {
            "installed": {
                "v0.5.0": {"path": "comfyui-versions/v0.5.0"},
            },
            "lastSelectedVersion": None,
            "defaultVersion": None,
        }
        metadata_manager.save_versions(test_data)

        assert metadata_manager.version_exists("v0.5.0") is True
        assert metadata_manager.version_exists("v0.6.0") is False
        assert metadata_manager.version_exists("nonexistent") is False


@pytest.mark.unit
class TestActiveVersion:
    """Tests for active version management."""

    def test_get_active_version_when_none(self, metadata_manager):
        """Test getting active version when none is set."""
        active = metadata_manager.get_active_version()
        assert active is None

    def test_set_and_get_active_version(self, metadata_manager):
        """Test setting and getting the active version."""
        # First, add a version to installed
        test_data = {
            "installed": {
                "v0.5.0": {"path": "comfyui-versions/v0.5.0"},
            },
            "lastSelectedVersion": None,
            "defaultVersion": None,
        }
        metadata_manager.save_versions(test_data)

        # Set active version
        success = metadata_manager.set_active_version("v0.5.0")
        assert success is True

        # Get and verify
        active = metadata_manager.get_active_version()
        assert active == "v0.5.0"

    def test_set_active_version_fails_for_nonexistent(self, metadata_manager):
        """Test that setting active version fails if version not installed."""
        success = metadata_manager.set_active_version("v0.5.0")
        assert success is False

        active = metadata_manager.get_active_version()
        assert active is None


@pytest.mark.unit
class TestVersionConfig:
    """Tests for version-specific config operations."""

    def test_load_nonexistent_version_config(self, metadata_manager):
        """Test loading a version config that doesn't exist."""
        config = metadata_manager.load_version_config("v0.5.0")
        assert config is None

    def test_save_and_load_version_config(self, metadata_manager):
        """Test saving and loading version-specific config."""
        test_config = {
            "python_version": "3.12.0",
            "custom_nodes_enabled": True,
            "startup_args": ["--listen", "0.0.0.0"],
        }

        # Save
        success = metadata_manager.save_version_config("v0.5.0", test_config)
        assert success is True

        # Verify file exists
        config_file = metadata_manager.version_configs_dir / "v0.5.0-config.json"
        assert config_file.exists()

        # Load and verify
        loaded = metadata_manager.load_version_config("v0.5.0")
        assert loaded == test_config

    def test_delete_version_config(self, metadata_manager):
        """Test deleting version-specific config."""
        test_config = {"test": "data"}
        metadata_manager.save_version_config("v0.5.0", test_config)

        # Verify it exists
        assert metadata_manager.load_version_config("v0.5.0") is not None

        # Delete
        success = metadata_manager.delete_version_config("v0.5.0")
        assert success is True

        # Verify deletion
        assert metadata_manager.load_version_config("v0.5.0") is None

    def test_delete_nonexistent_version_config(self, metadata_manager):
        """Test deleting a config that doesn't exist returns False."""
        success = metadata_manager.delete_version_config("nonexistent")
        assert success is False


@pytest.mark.unit
class TestOtherMetadata:
    """Tests for models, custom nodes, workflows, and GitHub cache."""

    def test_save_and_load_models(self, metadata_manager):
        """Test saving and loading models metadata."""
        test_data = {
            "checkpoints": ["model1.safetensors", "model2.ckpt"],
            "loras": ["lora1.safetensors"],
        }

        success = metadata_manager.save_models(test_data)
        assert success is True

        loaded = metadata_manager.load_models()
        assert loaded == test_data

    def test_save_and_load_custom_nodes(self, metadata_manager):
        """Test saving and loading custom nodes metadata."""
        test_data = {
            "installed": ["custom-node-1", "custom-node-2"],
        }

        success = metadata_manager.save_custom_nodes(test_data)
        assert success is True

        loaded = metadata_manager.load_custom_nodes()
        assert loaded == test_data

    def test_save_and_load_workflows(self, metadata_manager):
        """Test saving and loading workflows metadata."""
        test_data = {
            "workflows": ["workflow1.json", "workflow2.json"],
        }

        success = metadata_manager.save_workflows(test_data)
        assert success is True

        loaded = metadata_manager.load_workflows()
        assert loaded == test_data

    def test_save_and_load_github_cache(self, metadata_manager):
        """Test saving and loading GitHub releases cache."""
        test_data = {
            "releases": [
                {"tag_name": "v0.6.0", "prerelease": False},
                {"tag_name": "v0.5.0", "prerelease": False},
            ],
            "cached_at": "2024-01-15T10:00:00Z",
        }

        success = metadata_manager.save_github_cache(test_data)
        assert success is True

        loaded = metadata_manager.load_github_cache()
        assert loaded == test_data

    def test_load_github_cache_when_missing(self, metadata_manager):
        """Test loading GitHub cache when it doesn't exist."""
        loaded = metadata_manager.load_github_cache()
        assert loaded is None


@pytest.mark.unit
class TestAtomicWrites:
    """Tests for atomic write functionality."""

    def test_atomic_write_creates_temp_file(self, metadata_manager):
        """Test that atomic write uses a temporary file."""
        test_data = {"test": "data"}

        # Save data
        metadata_manager.save_versions(test_data)

        # Temp file should not exist after successful write
        temp_file = metadata_manager.versions_file.with_suffix(".tmp")
        assert not temp_file.exists()

        # Final file should exist
        assert metadata_manager.versions_file.exists()

    def test_corrupted_json_returns_default(self, metadata_manager, tmp_path):
        """Test that reading corrupted JSON returns default value."""
        # Create a corrupted JSON file
        corrupted_file = metadata_manager.versions_file
        corrupted_file.write_text("{ invalid json content }")

        # Should return default instead of crashing
        versions = metadata_manager.load_versions()
        assert "installed" in versions
        assert versions["installed"] == {}


@pytest.mark.unit
class TestEdgeCases:
    """Tests for edge cases and error handling."""

    def test_load_empty_file(self, metadata_manager):
        """Test loading a completely empty file."""
        # Create empty file
        metadata_manager.versions_file.write_text("")

        # Should return default
        versions = metadata_manager.load_versions()
        assert "installed" in versions

    def test_unicode_in_metadata(self, metadata_manager):
        """Test that Unicode characters are handled correctly."""
        test_data = {
            "installed": {
                "v0.5.0": {"path": "測試/路徑", "note": "日本語 🎨"},
            },
            "lastSelectedVersion": None,
            "defaultVersion": None,
        }

        success = metadata_manager.save_versions(test_data)
        assert success is True

        loaded = metadata_manager.load_versions()
        assert loaded == test_data
