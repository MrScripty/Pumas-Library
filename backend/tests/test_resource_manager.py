#!/usr/bin/env python3
"""
Unit tests for ResourceManager
Tests resource management, symlinks, models, custom nodes, and shared storage
"""

import json
import shutil
from pathlib import Path
from unittest.mock import Mock, patch

import pytest

from backend.metadata_manager import MetadataManager
from backend.models import ModelInfo, ModelsMetadata, RepairReport, ScanResult
from backend.resource_manager import ResourceManager

# ============================================================================
# FIXTURES
# ============================================================================


@pytest.fixture
def launcher_root(tmp_path):
    """Create a temporary launcher root directory"""
    return tmp_path / "launcher"


@pytest.fixture
def mock_metadata_manager():
    """Create a mock MetadataManager"""
    mock_mgr = Mock(spec=MetadataManager)
    mock_mgr.load_models.return_value = {}
    mock_mgr.save_models.return_value = None
    mock_mgr.load_versions.return_value = {"installed": {}}
    return mock_mgr


@pytest.fixture
def resource_manager(launcher_root, mock_metadata_manager):
    """Create a ResourceManager instance for testing"""
    launcher_root.mkdir(parents=True, exist_ok=True)
    return ResourceManager(launcher_root, mock_metadata_manager)


# ============================================================================
# INITIALIZATION TESTS
# ============================================================================


class TestResourceManagerInit:
    """Test ResourceManager initialization"""

    def test_init_sets_paths(self, launcher_root, mock_metadata_manager):
        """Test that initialization sets correct paths"""
        manager = ResourceManager(launcher_root, mock_metadata_manager)

        assert manager.launcher_root == launcher_root
        assert manager.metadata_manager == mock_metadata_manager
        assert manager.shared_dir == launcher_root / "shared-resources"
        assert manager.versions_dir == launcher_root / "comfyui-versions"
        assert manager.shared_models_dir == launcher_root / "shared-resources" / "models"
        assert (
            manager.shared_custom_nodes_cache_dir
            == launcher_root / "shared-resources" / "custom_nodes_cache"
        )
        assert manager.shared_user_dir == launcher_root / "shared-resources" / "user"
        assert (
            manager.shared_workflows_dir
            == launcher_root / "shared-resources" / "user" / "workflows"
        )
        assert (
            manager.shared_settings_dir == launcher_root / "shared-resources" / "user" / "settings"
        )

    def test_init_converts_string_path(self, tmp_path, mock_metadata_manager):
        """Test that string paths are converted to Path objects"""
        manager = ResourceManager(str(tmp_path), mock_metadata_manager)
        assert isinstance(manager.launcher_root, Path)


# ============================================================================
# SHARED STORAGE INITIALIZATION TESTS
# ============================================================================


class TestSharedStorageInitialization:
    """Test shared storage initialization"""

    def test_initialize_shared_storage_creates_directories(self, resource_manager):
        """Test that initialize_shared_storage creates all directories"""
        result = resource_manager.initialize_shared_storage()

        assert result is True
        assert resource_manager.shared_dir.exists()
        assert resource_manager.shared_models_dir.exists()
        assert resource_manager.shared_custom_nodes_cache_dir.exists()
        assert resource_manager.shared_user_dir.exists()
        assert resource_manager.shared_workflows_dir.exists()
        assert resource_manager.shared_settings_dir.exists()

    def test_initialize_shared_storage_idempotent(self, resource_manager):
        """Test that initializing twice doesn't fail"""
        result1 = resource_manager.initialize_shared_storage()
        result2 = resource_manager.initialize_shared_storage()

        assert result1 is True
        assert result2 is True

    def test_initialize_shared_storage_failure(self, resource_manager, mocker):
        """Test handling of directory creation failure"""
        mocker.patch("backend.resource_manager.ensure_directory", return_value=False)

        result = resource_manager.initialize_shared_storage()

        assert result is False


# ============================================================================
# MODEL DIRECTORY DISCOVERY TESTS
# ============================================================================


class TestModelDirectoryDiscovery:
    """Test model directory discovery"""

    def test_discover_model_directories_from_folder_paths(self, resource_manager, tmp_path):
        """Test discovering model directories from folder_paths.py"""
        comfyui_path = tmp_path / "comfyui"
        comfyui_path.mkdir()
        (comfyui_path / "comfy").mkdir()

        # Create folder_paths.py with model directories
        folder_paths = comfyui_path / "comfy" / "folder_paths.py"
        folder_paths.write_text(
            """
folder_names_and_paths["checkpoints"] = ([os.path.join(models_dir, "checkpoints")], supported_pt_extensions)
folder_names_and_paths["loras"] = ([os.path.join(models_dir, "loras")], supported_pt_extensions)
folder_names_and_paths["vae"] = ([os.path.join(models_dir, "vae")], supported_pt_extensions)
"""
        )

        result = resource_manager.discover_model_directories(comfyui_path)

        assert "checkpoints" in result
        assert "loras" in result
        assert "vae" in result
        assert len(result) == 3

    def test_discover_model_directories_missing_file(self, resource_manager, tmp_path):
        """Test fallback when folder_paths.py doesn't exist"""
        comfyui_path = tmp_path / "comfyui"
        comfyui_path.mkdir()

        result = resource_manager.discover_model_directories(comfyui_path)

        # Should return defaults
        assert "checkpoints" in result
        assert "loras" in result
        assert len(result) > 0

    def test_discover_model_directories_parse_error(self, resource_manager, tmp_path, mocker):
        """Test handling of parse errors"""
        comfyui_path = tmp_path / "comfyui"
        comfyui_path.mkdir()
        (comfyui_path / "comfy").mkdir()

        folder_paths = comfyui_path / "comfy" / "folder_paths.py"
        folder_paths.write_text("invalid syntax {{{")

        result = resource_manager.discover_model_directories(comfyui_path)

        # Should return defaults on parse error
        assert len(result) > 0

    def test_get_default_model_directories(self, resource_manager):
        """Test default model directories"""
        result = resource_manager._get_default_model_directories()

        assert "checkpoints" in result
        assert "loras" in result
        assert "vae" in result
        assert "controlnet" in result
        assert "upscale_models" in result
        assert len(result) == 14


# ============================================================================
# MODEL STRUCTURE SYNC TESTS
# ============================================================================


class TestModelStructureSync:
    """Test syncing shared model structure"""

    def test_sync_shared_model_structure_creates_directories(self, resource_manager, tmp_path):
        """Test that sync creates model directories"""
        resource_manager.initialize_shared_storage()

        comfyui_path = tmp_path / "comfyui"
        comfyui_path.mkdir()
        (comfyui_path / "comfy").mkdir()

        folder_paths = comfyui_path / "comfy" / "folder_paths.py"
        folder_paths.write_text(
            'folder_names_and_paths["checkpoints"] = ...\nfolder_names_and_paths["loras"] = ...'
        )

        result = resource_manager.sync_shared_model_structure(comfyui_path)

        assert result is True
        assert (resource_manager.shared_models_dir / "checkpoints").exists()
        assert (resource_manager.shared_models_dir / "loras").exists()

    def test_sync_shared_model_structure_preserves_existing(self, resource_manager, tmp_path):
        """Test that sync preserves existing directories"""
        resource_manager.initialize_shared_storage()

        # Create existing directory with a file
        checkpoints_dir = resource_manager.shared_models_dir / "checkpoints"
        checkpoints_dir.mkdir(parents=True)
        test_file = checkpoints_dir / "existing_model.safetensors"
        test_file.write_text("existing")

        comfyui_path = tmp_path / "comfyui"
        comfyui_path.mkdir()
        (comfyui_path / "comfy").mkdir()

        folder_paths = comfyui_path / "comfy" / "folder_paths.py"
        folder_paths.write_text('folder_names_and_paths["checkpoints"] = ...')

        result = resource_manager.sync_shared_model_structure(comfyui_path)

        assert result is True
        # Existing file should still be there
        assert test_file.exists()
        assert test_file.read_text() == "existing"


# ============================================================================
# SYMLINK SETUP TESTS
# ============================================================================


class TestSymlinkSetup:
    """Test version symlink setup"""

    def test_setup_version_symlinks_success(self, resource_manager):
        """Test successful symlink setup"""
        resource_manager.initialize_shared_storage()

        version_path = resource_manager.versions_dir / "v0.1.0"
        version_path.mkdir(parents=True)

        result = resource_manager.setup_version_symlinks("v0.1.0")

        assert result is True
        assert (version_path / "models").is_symlink()
        assert (version_path / "user").is_symlink()

    def test_setup_version_symlinks_version_not_found(self, resource_manager):
        """Test when version directory doesn't exist"""
        result = resource_manager.setup_version_symlinks("v9.9.9")

        assert result is False

    def test_setup_version_symlinks_replaces_existing(self, resource_manager):
        """Test that existing symlinks are replaced"""
        resource_manager.initialize_shared_storage()

        version_path = resource_manager.versions_dir / "v0.1.0"
        version_path.mkdir(parents=True)

        # Create existing symlinks
        (version_path / "models").symlink_to(resource_manager.shared_models_dir)
        (version_path / "user").symlink_to(resource_manager.shared_user_dir)

        result = resource_manager.setup_version_symlinks("v0.1.0")

        assert result is True
        assert (version_path / "models").is_symlink()
        assert (version_path / "user").is_symlink()


# ============================================================================
# SYMLINK VALIDATION AND REPAIR TESTS
# ============================================================================


class TestSymlinkValidation:
    """Test symlink validation and repair"""

    def test_validate_and_repair_symlinks_all_valid(self, resource_manager):
        """Test when all symlinks are valid"""
        resource_manager.initialize_shared_storage()

        version_path = resource_manager.versions_dir / "v0.1.0"
        version_path.mkdir(parents=True)

        resource_manager.setup_version_symlinks("v0.1.0")

        report = resource_manager.validate_and_repair_symlinks("v0.1.0")

        assert len(report["broken"]) == 0
        assert len(report["repaired"]) == 0
        assert len(report["removed"]) == 0

    def test_validate_and_repair_symlinks_missing(self, resource_manager):
        """Test repairing missing symlinks"""
        resource_manager.initialize_shared_storage()

        version_path = resource_manager.versions_dir / "v0.1.0"
        version_path.mkdir(parents=True)

        report = resource_manager.validate_and_repair_symlinks("v0.1.0")

        # Should create missing symlinks
        assert len(report["repaired"]) == 2
        assert (version_path / "models").is_symlink()
        assert (version_path / "user").is_symlink()

    def test_validate_and_repair_symlinks_broken(self, resource_manager):
        """Test repairing broken symlinks"""
        resource_manager.initialize_shared_storage()

        version_path = resource_manager.versions_dir / "v0.1.0"
        version_path.mkdir(parents=True)

        # Create broken symlinks
        (version_path / "models").symlink_to("/nonexistent/path")
        (version_path / "user").symlink_to("/nonexistent/path")

        report = resource_manager.validate_and_repair_symlinks("v0.1.0")

        assert len(report["broken"]) == 2
        assert len(report["repaired"]) == 2

    def test_validate_and_repair_symlinks_version_not_found(self, resource_manager):
        """Test when version doesn't exist"""
        report = resource_manager.validate_and_repair_symlinks("v9.9.9")

        assert len(report["broken"]) == 0
        assert len(report["repaired"]) == 0
        assert len(report["removed"]) == 0


# ============================================================================
# FILE MIGRATION TESTS
# ============================================================================


class TestFileMigration:
    """Test migrating existing files to shared storage"""

    def test_migrate_existing_files_models(self, resource_manager, tmp_path):
        """Test migrating model files"""
        resource_manager.initialize_shared_storage()

        version_path = resource_manager.versions_dir / "v0.1.0"
        version_path.mkdir(parents=True)

        # Create real models directory with files
        models_dir = version_path / "models"
        checkpoints_dir = models_dir / "checkpoints"
        checkpoints_dir.mkdir(parents=True)

        model_file = checkpoints_dir / "model.safetensors"
        model_file.write_text("model data")

        # Ensure shared directory exists
        (resource_manager.shared_models_dir / "checkpoints").mkdir(parents=True)

        files_moved, conflicts, conflict_paths = resource_manager.migrate_existing_files(
            version_path
        )

        assert files_moved == 1
        assert conflicts == 0
        assert (resource_manager.shared_models_dir / "checkpoints" / "model.safetensors").exists()

    def test_migrate_existing_files_conflict(self, resource_manager):
        """Test handling file conflicts during migration"""
        resource_manager.initialize_shared_storage()

        version_path = resource_manager.versions_dir / "v0.1.0"
        version_path.mkdir(parents=True)

        # Create model in version directory
        models_dir = version_path / "models"
        checkpoints_dir = models_dir / "checkpoints"
        checkpoints_dir.mkdir(parents=True)
        model_file = checkpoints_dir / "model.safetensors"
        model_file.write_text("version data")

        # Create same model in shared storage
        shared_checkpoints = resource_manager.shared_models_dir / "checkpoints"
        shared_checkpoints.mkdir(parents=True)
        (shared_checkpoints / "model.safetensors").write_text("shared data")

        files_moved, conflicts, conflict_paths = resource_manager.migrate_existing_files(
            version_path, auto_merge=False
        )

        assert files_moved == 0
        assert conflicts == 1
        assert len(conflict_paths) == 1

    def test_migrate_existing_files_workflows(self, resource_manager):
        """Test migrating workflow files"""
        resource_manager.initialize_shared_storage()

        version_path = resource_manager.versions_dir / "v0.1.0"
        version_path.mkdir(parents=True)

        # Create real user directory with workflows
        user_dir = version_path / "user"
        workflows_dir = user_dir / "workflows"
        workflows_dir.mkdir(parents=True)

        workflow_file = workflows_dir / "workflow.json"
        workflow_file.write_text('{"nodes": []}')

        files_moved, conflicts, conflict_paths = resource_manager.migrate_existing_files(
            version_path
        )

        assert files_moved == 1
        assert (resource_manager.shared_workflows_dir / "workflow.json").exists()

    def test_migrate_existing_files_cleans_empty_dirs(self, resource_manager):
        """Test that empty directories are removed after migration"""
        resource_manager.initialize_shared_storage()

        version_path = resource_manager.versions_dir / "v0.1.0"
        version_path.mkdir(parents=True)

        # Create models directory with files
        models_dir = version_path / "models"
        checkpoints_dir = models_dir / "checkpoints"
        checkpoints_dir.mkdir(parents=True)
        (checkpoints_dir / "model.safetensors").write_text("data")

        (resource_manager.shared_models_dir / "checkpoints").mkdir(parents=True)

        resource_manager.migrate_existing_files(version_path)

        # Empty directories should be removed
        assert not models_dir.exists()

    def test_migrate_existing_files_skips_symlinks(self, resource_manager):
        """Test that symlinks are not migrated"""
        resource_manager.initialize_shared_storage()

        version_path = resource_manager.versions_dir / "v0.1.0"
        version_path.mkdir(parents=True)

        # Create symlinks instead of real directories
        resource_manager.setup_version_symlinks("v0.1.0")

        files_moved, conflicts, conflict_paths = resource_manager.migrate_existing_files(
            version_path
        )

        assert files_moved == 0


# ============================================================================
# MODEL MANAGEMENT TESTS
# ============================================================================


class TestModelManagement:
    """Test model management operations"""

    def test_get_models(self, resource_manager, mock_metadata_manager):
        """Test getting models from metadata"""
        mock_metadata_manager.load_models.return_value = {
            "checkpoints/model.safetensors": {
                "path": "checkpoints/model.safetensors",
                "size": 1000,
            }
        }

        result = resource_manager.get_models()

        assert "checkpoints/model.safetensors" in result
        mock_metadata_manager.load_models.assert_called_once()

    def test_add_model_success(self, resource_manager, tmp_path):
        """Test adding a model to shared storage"""
        resource_manager.initialize_shared_storage()

        source_file = tmp_path / "test_model.safetensors"
        source_file.write_text("model data")

        result = resource_manager.add_model(source_file, "checkpoints")

        assert result is True
        assert (
            resource_manager.shared_models_dir / "checkpoints" / "test_model.safetensors"
        ).exists()

    def test_add_model_source_not_found(self, resource_manager, tmp_path):
        """Test adding non-existent model"""
        result = resource_manager.add_model(tmp_path / "missing.safetensors", "checkpoints")

        assert result is False

    def test_add_model_already_exists(self, resource_manager, tmp_path):
        """Test adding model that already exists"""
        resource_manager.initialize_shared_storage()

        source_file = tmp_path / "test_model.safetensors"
        source_file.write_text("model data")

        # Add once
        resource_manager.add_model(source_file, "checkpoints")

        # Try to add again
        result = resource_manager.add_model(source_file, "checkpoints")

        assert result is False

    def test_add_model_updates_metadata(self, resource_manager, tmp_path, mocker):
        """Test that adding model updates metadata"""
        resource_manager.initialize_shared_storage()

        source_file = tmp_path / "test_model.safetensors"
        source_file.write_text("model data")

        # Mock hash calculation
        mocker.patch("backend.resource_manager.calculate_file_hash", return_value="abc123")

        result = resource_manager.add_model(source_file, "checkpoints", update_metadata=True)

        assert result is True
        # Verify save_models was called
        resource_manager.metadata_manager.save_models.assert_called_once()

    def test_remove_model_success(self, resource_manager):
        """Test removing a model"""
        resource_manager.initialize_shared_storage()

        # Create model file
        model_path = resource_manager.shared_models_dir / "checkpoints" / "model.safetensors"
        model_path.parent.mkdir(parents=True, exist_ok=True)
        model_path.write_text("model data")

        result = resource_manager.remove_model("checkpoints/model.safetensors")

        assert result is True
        assert not model_path.exists()

    def test_remove_model_not_found(self, resource_manager):
        """Test removing non-existent model"""
        result = resource_manager.remove_model("checkpoints/missing.safetensors")

        assert result is False

    def test_remove_model_updates_metadata(self, resource_manager, mock_metadata_manager):
        """Test that removing model updates metadata"""
        resource_manager.initialize_shared_storage()

        # Create model file
        model_path = resource_manager.shared_models_dir / "checkpoints" / "model.safetensors"
        model_path.parent.mkdir(parents=True, exist_ok=True)
        model_path.write_text("model data")

        # Set up metadata
        mock_metadata_manager.load_models.return_value = {
            "checkpoints/model.safetensors": {"path": "checkpoints/model.safetensors"}
        }

        resource_manager.remove_model("checkpoints/model.safetensors")

        # Verify metadata was saved
        mock_metadata_manager.save_models.assert_called_once()


# ============================================================================
# SHARED STORAGE SCAN TESTS
# ============================================================================


class TestSharedStorageScan:
    """Test scanning shared storage"""

    def test_scan_shared_storage_empty(self, resource_manager):
        """Test scanning empty shared storage"""
        resource_manager.initialize_shared_storage()

        result = resource_manager.scan_shared_storage()

        assert result["modelsFound"] == 0
        assert result["workflowsFound"] == 0
        assert result["customNodesFound"] == 0
        assert result["totalSize"] == 0

    def test_scan_shared_storage_with_models(self, resource_manager):
        """Test scanning with models"""
        resource_manager.initialize_shared_storage()

        # Create model files
        checkpoints_dir = resource_manager.shared_models_dir / "checkpoints"
        checkpoints_dir.mkdir(parents=True)
        (checkpoints_dir / "model1.safetensors").write_text("data1")
        (checkpoints_dir / "model2.safetensors").write_text("data2")

        result = resource_manager.scan_shared_storage()

        assert result["modelsFound"] == 2
        assert result["totalSize"] > 0

    def test_scan_shared_storage_with_workflows(self, resource_manager):
        """Test scanning with workflows"""
        resource_manager.initialize_shared_storage()

        # Create workflow files
        (resource_manager.shared_workflows_dir / "workflow1.json").write_text('{"nodes":[]}')
        (resource_manager.shared_workflows_dir / "workflow2.json").write_text('{"nodes":[]}')

        result = resource_manager.scan_shared_storage()

        assert result["workflowsFound"] == 2
        assert result["totalSize"] > 0


# ============================================================================
# CUSTOM NODE MANAGEMENT TESTS
# ============================================================================


class TestCustomNodeManagement:
    """Test custom node management"""

    def test_get_version_custom_nodes_dir(self, resource_manager):
        """Test getting custom nodes directory path"""
        result = resource_manager.get_version_custom_nodes_dir("v0.1.0")

        assert result == resource_manager.versions_dir / "v0.1.0" / "custom_nodes"

    def test_list_version_custom_nodes_empty(self, resource_manager):
        """Test listing custom nodes when directory doesn't exist"""
        result = resource_manager.list_version_custom_nodes("v0.1.0")

        assert result == []

    def test_list_version_custom_nodes_with_nodes(self, resource_manager):
        """Test listing custom nodes"""
        custom_nodes_dir = resource_manager.get_version_custom_nodes_dir("v0.1.0")
        custom_nodes_dir.mkdir(parents=True)

        # Create custom node directories
        (custom_nodes_dir / "ComfyUI-CustomNode1").mkdir()
        (custom_nodes_dir / "ComfyUI-CustomNode2").mkdir()
        (custom_nodes_dir / ".hidden").mkdir()  # Should be ignored

        result = resource_manager.list_version_custom_nodes("v0.1.0")

        assert len(result) == 2
        assert "ComfyUI-CustomNode1" in result
        assert "ComfyUI-CustomNode2" in result
        assert ".hidden" not in result

    def test_install_custom_node_success(self, resource_manager, mocker):
        """Test installing a custom node"""
        mock_run_command = mocker.patch("backend.utils.run_command")
        mock_run_command.return_value = (True, "", "")

        result = resource_manager.install_custom_node(
            "https://github.com/user/ComfyUI-CustomNode.git", "v0.1.0"
        )

        assert result is True
        mock_run_command.assert_called_once()

    def test_install_custom_node_already_exists(self, resource_manager, mocker):
        """Test installing when node already exists"""
        custom_nodes_dir = resource_manager.get_version_custom_nodes_dir("v0.1.0")
        custom_nodes_dir.mkdir(parents=True)
        (custom_nodes_dir / "ComfyUI-CustomNode").mkdir()

        result = resource_manager.install_custom_node(
            "https://github.com/user/ComfyUI-CustomNode.git", "v0.1.0"
        )

        assert result is False

    def test_install_custom_node_git_failure(self, resource_manager, mocker):
        """Test handling git clone failure"""
        mock_run_command = mocker.patch("backend.utils.run_command")
        mock_run_command.return_value = (False, "", "git error")

        result = resource_manager.install_custom_node(
            "https://github.com/user/ComfyUI-CustomNode.git", "v0.1.0"
        )

        assert result is False

    def test_update_custom_node_success(self, resource_manager, mocker):
        """Test updating a custom node"""
        custom_nodes_dir = resource_manager.get_version_custom_nodes_dir("v0.1.0")
        custom_nodes_dir.mkdir(parents=True)
        node_dir = custom_nodes_dir / "ComfyUI-CustomNode"
        node_dir.mkdir()
        (node_dir / ".git").mkdir()

        mock_run_command = mocker.patch("backend.utils.run_command")
        mock_run_command.return_value = (True, "Already up to date.", "")

        result = resource_manager.update_custom_node("ComfyUI-CustomNode", "v0.1.0")

        assert result is True
        mock_run_command.assert_called_once()

    def test_update_custom_node_not_found(self, resource_manager):
        """Test updating non-existent custom node"""
        result = resource_manager.update_custom_node("NonExistent", "v0.1.0")

        assert result is False

    def test_update_custom_node_not_git_repo(self, resource_manager):
        """Test updating node that's not a git repo"""
        custom_nodes_dir = resource_manager.get_version_custom_nodes_dir("v0.1.0")
        custom_nodes_dir.mkdir(parents=True)
        (custom_nodes_dir / "ComfyUI-CustomNode").mkdir()
        # No .git directory

        result = resource_manager.update_custom_node("ComfyUI-CustomNode", "v0.1.0")

        assert result is False

    def test_remove_custom_node_success(self, resource_manager):
        """Test removing a custom node"""
        custom_nodes_dir = resource_manager.get_version_custom_nodes_dir("v0.1.0")
        custom_nodes_dir.mkdir(parents=True)
        node_dir = custom_nodes_dir / "ComfyUI-CustomNode"
        node_dir.mkdir()
        (node_dir / "file.py").write_text("code")

        result = resource_manager.remove_custom_node("ComfyUI-CustomNode", "v0.1.0")

        assert result is True
        assert not node_dir.exists()

    def test_remove_custom_node_not_found(self, resource_manager):
        """Test removing non-existent custom node"""
        result = resource_manager.remove_custom_node("NonExistent", "v0.1.0")

        assert result is False


# ============================================================================
# CUSTOM NODE CACHING TESTS
# ============================================================================


class TestCustomNodeCaching:
    """Test custom node repository caching"""

    def test_cache_custom_node_repo_new(self, resource_manager, mocker):
        """Test caching a new repository"""
        mock_run_command = mocker.patch("backend.utils.run_command")
        mock_run_command.return_value = (True, "", "")

        result = resource_manager.cache_custom_node_repo(
            "https://github.com/user/ComfyUI-CustomNode.git"
        )

        assert result is not None
        assert result.name == "ComfyUI-CustomNode.git"
        mock_run_command.assert_called_once()

    def test_cache_custom_node_repo_update_existing(self, resource_manager, mocker):
        """Test updating an existing cached repository"""
        # Create existing cache
        cache_dir = resource_manager.shared_custom_nodes_cache_dir
        cache_dir.mkdir(parents=True)
        repo_cache = cache_dir / "ComfyUI-CustomNode.git"
        repo_cache.mkdir()
        (repo_cache / "HEAD").write_text("ref: refs/heads/main")

        mock_run_command = mocker.patch("backend.utils.run_command")
        mock_run_command.return_value = (True, "", "")

        result = resource_manager.cache_custom_node_repo(
            "https://github.com/user/ComfyUI-CustomNode.git"
        )

        assert result == repo_cache
        mock_run_command.assert_called_once()
        # Should have called git fetch
        assert "fetch" in mock_run_command.call_args[0][0][1]

    def test_cache_custom_node_repo_clone_failure(self, resource_manager, mocker):
        """Test handling clone failure"""
        mock_run_command = mocker.patch("backend.utils.run_command")
        mock_run_command.return_value = (False, "", "git error")

        result = resource_manager.cache_custom_node_repo(
            "https://github.com/user/ComfyUI-CustomNode.git"
        )

        assert result is None
