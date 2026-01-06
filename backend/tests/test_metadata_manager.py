"""
Unit tests for backend/metadata_manager.py

Tests for MetadataManager initialization, JSON operations, and metadata persistence.
"""

import json
from pathlib import Path
from unittest.mock import Mock, mock_open, patch

import pytest

from backend.exceptions import MetadataError, ResourceError
from backend.metadata_manager import MetadataManager

# ============================================================================
# FIXTURES
# ============================================================================


@pytest.fixture
def metadata_manager(tmp_path):
    """Create a MetadataManager instance for testing"""
    launcher_data_dir = tmp_path / "launcher-data"
    return MetadataManager(launcher_data_dir)


@pytest.fixture
def sample_versions_data():
    """Sample versions metadata"""
    return {
        "installed": {
            "v0.1.0": {
                "installedDate": "2024-01-01T00:00:00Z",
                "installPath": "/path/to/v0.1.0",
            }
        },
        "lastSelectedVersion": "v0.1.0",
        "defaultVersion": None,
    }


@pytest.fixture
def sample_version_config():
    """Sample version config"""
    return {
        "tag": "v0.1.0",
        "pythonVersion": "3.11",
        "extraArgs": ["--preview-method", "auto"],
    }


# ============================================================================
# INITIALIZATION TESTS
# ============================================================================


class TestMetadataManagerInit:
    """Test MetadataManager initialization"""

    def test_init_creates_directories(self, tmp_path):
        """Test that initialization creates all required directories"""
        launcher_data_dir = tmp_path / "launcher-data"
        mm = MetadataManager(launcher_data_dir)

        assert mm.metadata_dir.exists()
        assert mm.config_dir.exists()
        assert mm.cache_dir.exists()
        assert mm.version_configs_dir.exists()

    def test_init_sets_file_paths(self, tmp_path):
        """Test that initialization sets correct file paths"""
        launcher_data_dir = tmp_path / "launcher-data"
        mm = MetadataManager(launcher_data_dir)

        assert mm.versions_file == mm.metadata_dir / "versions.json"
        assert mm.models_file == mm.metadata_dir / "models.json"
        assert mm.custom_nodes_file == mm.metadata_dir / "custom_nodes.json"
        assert mm.workflows_file == mm.metadata_dir / "workflows.json"
        assert mm.github_cache_file == mm.cache_dir / "github-releases.json"

    def test_init_creates_write_lock(self, tmp_path):
        """Test that initialization creates threading lock"""
        launcher_data_dir = tmp_path / "launcher-data"
        mm = MetadataManager(launcher_data_dir)

        assert hasattr(mm._write_lock, "acquire")
        assert hasattr(mm._write_lock, "release")

    def test_init_with_existing_directories(self, tmp_path):
        """Test initialization with pre-existing directories"""
        launcher_data_dir = tmp_path / "launcher-data"
        launcher_data_dir.mkdir(parents=True, exist_ok=True)
        (launcher_data_dir / "metadata").mkdir(parents=True, exist_ok=True)

        mm = MetadataManager(launcher_data_dir)

        assert mm.metadata_dir.exists()
        assert mm.config_dir.exists()

    def test_init_validates_launcher_data_dir(self, tmp_path):
        """Test that launcher_data_dir is converted to Path"""
        launcher_data_dir = str(tmp_path / "launcher-data")
        mm = MetadataManager(launcher_data_dir)

        assert isinstance(mm.launcher_data_dir, Path)


# ============================================================================
# JSON OPERATIONS TESTS
# ============================================================================


class TestMetadataManagerJSONOperations:
    """Test JSON read/write operations"""

    def test_read_json_success(self, metadata_manager, tmp_path):
        """Test reading valid JSON file"""
        test_file = tmp_path / "test.json"
        test_data = {"key": "value", "number": 42}
        test_file.write_text(json.dumps(test_data))

        result = metadata_manager._read_json(test_file)

        assert result == test_data

    def test_read_json_file_not_found_returns_default(self, metadata_manager, tmp_path):
        """Test that non-existent file returns default value"""
        test_file = tmp_path / "nonexistent.json"
        default = {"default": "value"}

        result = metadata_manager._read_json(test_file, default)

        assert result == default

    def test_read_json_file_not_found_returns_empty_dict(self, metadata_manager, tmp_path):
        """Test that non-existent file returns empty dict when no default"""
        test_file = tmp_path / "nonexistent.json"

        result = metadata_manager._read_json(test_file)

        assert result == {}

    def test_read_json_invalid_json_raises_error(self, metadata_manager, tmp_path):
        """Test that invalid JSON raises MetadataError when no default"""
        test_file = tmp_path / "invalid.json"
        test_file.write_text("{ invalid json }")

        with pytest.raises(MetadataError, match="Failed to parse JSON"):
            metadata_manager._read_json(test_file)

    def test_read_json_invalid_json_returns_default(self, metadata_manager, tmp_path):
        """Test that invalid JSON returns default if provided"""
        test_file = tmp_path / "invalid.json"
        test_file.write_text("{ invalid json }")
        default = {"fallback": "data"}

        result = metadata_manager._read_json(test_file, default)

        assert result == default

    def test_read_json_io_error_with_no_default(self, metadata_manager, tmp_path):
        """Test that I/O error raises ResourceError when no default"""
        test_file = tmp_path / "test.json"
        test_file.write_text("{}")
        test_file.chmod(0o000)  # Remove read permissions

        try:
            with pytest.raises(ResourceError, match="Failed to read metadata file"):
                metadata_manager._read_json(test_file)
        finally:
            test_file.chmod(0o644)  # Restore permissions

    def test_write_json_success(self, metadata_manager, tmp_path):
        """Test writing JSON file successfully"""
        test_file = tmp_path / "test.json"
        test_data = {"key": "value", "number": 42}

        result = metadata_manager._write_json(test_file, test_data)

        assert result is True
        assert test_file.exists()
        saved_data = json.loads(test_file.read_text())
        assert saved_data == test_data

    def test_write_json_serialization_error(self, metadata_manager, tmp_path):
        """Test that non-serializable data raises MetadataError"""
        test_file = tmp_path / "test.json"
        # Create non-serializable object
        non_serializable = {"func": lambda x: x}

        with pytest.raises(MetadataError, match="Failed to serialize metadata"):
            metadata_manager._write_json(test_file, non_serializable)

    def test_write_json_creates_backup(self, metadata_manager, tmp_path):
        """Test that atomic write creates backup of existing file"""
        test_file = tmp_path / "test.json"
        original_data = {"original": "data"}
        test_file.write_text(json.dumps(original_data))

        new_data = {"new": "data"}
        metadata_manager._write_json(test_file, new_data)

        # Backup should exist
        backup_file = test_file.with_suffix(".json.bak")
        assert backup_file.exists()
        backup_data = json.loads(backup_file.read_text())
        assert backup_data == original_data


# ============================================================================
# VERSIONS METADATA TESTS
# ============================================================================


class TestVersionsMetadata:
    """Test versions metadata operations"""

    def test_load_versions_empty_default(self, metadata_manager):
        """Test loading versions when file doesn't exist returns default"""
        result = metadata_manager.load_versions()

        assert result == {
            "installed": {},
            "lastSelectedVersion": None,
            "defaultVersion": None,
        }

    def test_load_versions_with_existing_data(self, metadata_manager, sample_versions_data):
        """Test loading versions from existing file"""
        metadata_manager.versions_file.write_text(json.dumps(sample_versions_data))

        result = metadata_manager.load_versions()

        assert result == sample_versions_data
        assert "v0.1.0" in result["installed"]

    def test_save_versions_atomic_write(self, metadata_manager, sample_versions_data):
        """Test saving versions uses atomic write"""
        result = metadata_manager.save_versions(sample_versions_data)

        assert result is True
        assert metadata_manager.versions_file.exists()

        loaded = json.loads(metadata_manager.versions_file.read_text())
        assert loaded == sample_versions_data

    def test_save_versions_creates_backup(self, metadata_manager, sample_versions_data):
        """Test that save_versions creates backup of existing file"""
        original_data = {
            "installed": {},
            "lastSelectedVersion": None,
            "defaultVersion": None,
        }
        metadata_manager.versions_file.write_text(json.dumps(original_data))

        metadata_manager.save_versions(sample_versions_data)

        backup_file = metadata_manager.versions_file.with_suffix(".json.bak")
        assert backup_file.exists()


# ============================================================================
# VERSION CONFIG TESTS
# ============================================================================


class TestVersionConfig:
    """Test version-specific config operations"""

    def test_load_version_config_exists(self, metadata_manager, sample_version_config):
        """Test loading existing version config"""
        tag = "v0.1.0"
        config_file = metadata_manager.version_configs_dir / f"{tag}-config.json"
        config_file.write_text(json.dumps(sample_version_config))

        result = metadata_manager.load_version_config(tag)

        assert result == sample_version_config

    def test_load_version_config_not_found(self, metadata_manager):
        """Test loading non-existent version config returns None"""
        result = metadata_manager.load_version_config("v9.9.9")

        assert result is None

    def test_save_version_config(self, metadata_manager, sample_version_config):
        """Test saving version config"""
        tag = "v0.1.0"

        result = metadata_manager.save_version_config(tag, sample_version_config)

        assert result is True
        config_file = metadata_manager.version_configs_dir / f"{tag}-config.json"
        assert config_file.exists()

        loaded = json.loads(config_file.read_text())
        assert loaded == sample_version_config

    def test_delete_version_config(self, metadata_manager, sample_version_config):
        """Test deleting version config"""
        tag = "v0.1.0"
        config_file = metadata_manager.version_configs_dir / f"{tag}-config.json"
        config_file.write_text(json.dumps(sample_version_config))

        result = metadata_manager.delete_version_config(tag)

        assert result is True
        assert not config_file.exists()

    def test_delete_version_config_not_exists(self, metadata_manager):
        """Test deleting non-existent config returns False"""
        result = metadata_manager.delete_version_config("v9.9.9")

        assert result is False


# ============================================================================
# RESOURCE METADATA TESTS
# ============================================================================


class TestResourceMetadata:
    """Test models, custom nodes, and workflows metadata"""

    def test_load_save_models_metadata(self, metadata_manager):
        """Test loading and saving models metadata"""
        models_data = {
            "checkpoints": {
                "sd-v1-5.safetensors": {
                    "path": "/shared/models/checkpoints/sd-v1-5.safetensors",
                    "size": 4265380512,
                }
            }
        }

        # Save
        result = metadata_manager.save_models(models_data)
        assert result is True

        # Load
        loaded = metadata_manager.load_models()
        assert loaded == models_data

    def test_load_save_custom_nodes_metadata(self, metadata_manager):
        """Test loading and saving custom nodes metadata"""
        nodes_data = {
            "ComfyUI-Manager": {
                "installed": True,
                "path": "/custom_nodes/ComfyUI-Manager",
            }
        }

        result = metadata_manager.save_custom_nodes(nodes_data)
        assert result is True

        loaded = metadata_manager.load_custom_nodes()
        assert loaded == nodes_data

    def test_load_save_workflows_metadata(self, metadata_manager):
        """Test loading and saving workflows metadata"""
        workflows_data = {
            "workflow1.json": {
                "created": "2024-01-01T00:00:00Z",
                "modified": "2024-01-02T00:00:00Z",
            }
        }

        result = metadata_manager.save_workflows(workflows_data)
        assert result is True

        loaded = metadata_manager.load_workflows()
        assert loaded == workflows_data

    def test_load_models_empty_default(self, metadata_manager):
        """Test loading models when file doesn't exist"""
        result = metadata_manager.load_models()
        assert result == {}

    def test_load_custom_nodes_empty_default(self, metadata_manager):
        """Test loading custom nodes when file doesn't exist"""
        result = metadata_manager.load_custom_nodes()
        assert result == {}

    def test_load_workflows_empty_default(self, metadata_manager):
        """Test loading workflows when file doesn't exist"""
        result = metadata_manager.load_workflows()
        assert result == {}


# ============================================================================
# GITHUB CACHE TESTS
# ============================================================================


class TestGitHubCache:
    """Test GitHub releases cache operations"""

    def test_load_github_cache(self, metadata_manager):
        """Test loading GitHub cache"""
        cache_data = {
            "lastFetched": "2024-01-01T00:00:00Z",
            "ttl": 3600,
            "releases": [{"tag_name": "v0.1.0"}],
        }
        metadata_manager.github_cache_file.write_text(json.dumps(cache_data))

        result = metadata_manager.load_github_cache()

        assert result == cache_data

    def test_save_github_cache(self, metadata_manager):
        """Test saving GitHub cache"""
        cache_data = {
            "lastFetched": "2024-01-01T00:00:00Z",
            "ttl": 3600,
            "releases": [{"tag_name": "v0.1.0"}],
        }

        result = metadata_manager.save_github_cache(cache_data)

        assert result is True
        assert metadata_manager.github_cache_file.exists()

        loaded = json.loads(metadata_manager.github_cache_file.read_text())
        assert loaded == cache_data

    def test_github_cache_not_found(self, metadata_manager):
        """Test loading GitHub cache when file doesn't exist"""
        result = metadata_manager.load_github_cache()

        assert result is None


# ============================================================================
# UTILITY METHODS TESTS
# ============================================================================


class TestUtilityMethods:
    """Test utility methods"""

    def test_get_all_version_tags(self, metadata_manager, sample_versions_data):
        """Test getting all installed version tags"""
        metadata_manager.versions_file.write_text(json.dumps(sample_versions_data))

        result = metadata_manager.get_all_version_tags()

        assert result == ["v0.1.0"]

    def test_get_all_version_tags_empty(self, metadata_manager):
        """Test getting version tags when none installed"""
        result = metadata_manager.get_all_version_tags()

        assert result == []

    def test_version_exists(self, metadata_manager, sample_versions_data):
        """Test checking if version exists"""
        metadata_manager.versions_file.write_text(json.dumps(sample_versions_data))

        assert metadata_manager.version_exists("v0.1.0") is True
        assert metadata_manager.version_exists("v9.9.9") is False

    def test_get_active_version(self, metadata_manager, sample_versions_data):
        """Test getting active version"""
        metadata_manager.versions_file.write_text(json.dumps(sample_versions_data))

        result = metadata_manager.get_active_version()

        assert result == "v0.1.0"

    def test_get_active_version_none(self, metadata_manager):
        """Test getting active version when none set"""
        result = metadata_manager.get_active_version()

        assert result is None

    def test_set_active_version(self, metadata_manager, sample_versions_data):
        """Test setting active version"""
        metadata_manager.versions_file.write_text(json.dumps(sample_versions_data))

        result = metadata_manager.set_active_version("v0.1.0")

        assert result is True

        versions = metadata_manager.load_versions()
        assert versions["lastSelectedVersion"] == "v0.1.0"

    def test_set_active_version_not_installed(self, metadata_manager):
        """Test setting active version for non-installed version fails"""
        result = metadata_manager.set_active_version("v9.9.9")

        assert result is False
