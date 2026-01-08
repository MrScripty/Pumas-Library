#!/usr/bin/env python3
"""
Metadata Manager for ComfyUI Version Manager
Handles reading/writing metadata JSON files with atomic updates and validation
"""

import json
import threading
from pathlib import Path
from typing import Any, Dict, Optional, cast

from backend.exceptions import MetadataError, ResourceError
from backend.file_utils import atomic_write_json
from backend.logging_config import get_logger
from backend.models import (
    CustomNodesMetadata,
    GitHubReleasesCache,
    ModelsMetadata,
    VersionConfig,
    VersionsMetadata,
    WorkflowsMetadata,
)

logger = get_logger(__name__)


class MetadataManager:
    """Manages all metadata files for the version manager"""

    def __init__(self, launcher_data_dir: Path):
        """
        Initialize metadata manager

        Args:
            launcher_data_dir: Path to launcher-data directory
        """
        self.launcher_data_dir = Path(launcher_data_dir)
        self.metadata_dir = self.launcher_data_dir / "metadata"
        self.config_dir = self.launcher_data_dir / "config"
        self.cache_dir = self.launcher_data_dir / "cache"
        self.version_configs_dir = self.config_dir / "version-configs"
        self._write_lock = threading.Lock()

        # Ensure directories exist
        self.metadata_dir.mkdir(parents=True, exist_ok=True)
        self.config_dir.mkdir(parents=True, exist_ok=True)
        self.cache_dir.mkdir(parents=True, exist_ok=True)
        self.version_configs_dir.mkdir(parents=True, exist_ok=True)

        # Define file paths
        self.versions_file = self.metadata_dir / "versions.json"
        self.models_file = self.metadata_dir / "models.json"
        self.custom_nodes_file = self.metadata_dir / "custom_nodes.json"
        self.workflows_file = self.metadata_dir / "workflows.json"
        self.github_cache_file = self.cache_dir / "github-releases.json"

    # ==================== Generic JSON Operations ====================

    def _read_json(self, file_path: Path, default: Any = None) -> Any:
        """
        Read JSON file with error handling

        Args:
            file_path: Path to JSON file
            default: Default value if file doesn't exist or is invalid

        Returns:
            Parsed JSON data or default value

        Raises:
            MetadataError: If JSON parsing fails and no default is provided
            ResourceError: If file I/O fails and no default is provided
        """
        if not file_path.exists():
            return default if default is not None else {}

        try:
            with open(file_path, "r", encoding="utf-8") as f:
                return json.load(f)
        except json.JSONDecodeError as e:
            logger.error(f"Error parsing JSON in {file_path}: {e}", exc_info=True)
            if default is None:
                raise MetadataError(f"Failed to parse JSON: {e}", file_path=str(file_path)) from e
            return default
        except IOError as e:
            logger.error(f"Error reading {file_path}: {e}", exc_info=True)
            if default is None:
                raise ResourceError(
                    f"Failed to read metadata file: {e}", resource_type="file"
                ) from e
            return default

    def _write_json(self, file_path: Path, data: Any) -> bool:
        """
        Write JSON file atomically (write to temp file, then rename)

        Args:
            file_path: Path to JSON file
            data: Data to write

        Returns:
            True if successful, False otherwise

        Raises:
            MetadataError: If JSON serialization fails
            ResourceError: If file I/O fails
        """
        try:
            atomic_write_json(file_path, data, lock=self._write_lock, keep_backup=True)
            return True
        except TypeError as e:
            # JSON serialization errors
            logger.error(f"Error serializing data for {file_path}: {e}", exc_info=True)
            raise MetadataError(
                f"Failed to serialize metadata to JSON: {e}", file_path=str(file_path)
            ) from e
        except ValueError as e:
            # JSON serialization errors
            logger.error(f"Error serializing data for {file_path}: {e}", exc_info=True)
            raise MetadataError(
                f"Failed to serialize metadata to JSON: {e}", file_path=str(file_path)
            ) from e
        except OSError as e:
            # File I/O errors
            logger.error(f"Error writing {file_path}: {e}", exc_info=True)
            raise ResourceError(f"Failed to write metadata file: {e}", resource_type="file") from e

    # ==================== Versions Metadata ====================

    def load_versions(self) -> VersionsMetadata:
        """
        Load versions.json

        Returns:
            VersionsMetadata with installed versions
        """
        default: VersionsMetadata = {
            "installed": {},
            "lastSelectedVersion": None,
            "defaultVersion": None,
        }
        return cast(VersionsMetadata, self._read_json(self.versions_file, default))

    def save_versions(self, data: VersionsMetadata) -> bool:
        """
        Save versions.json atomically

        Args:
            data: VersionsMetadata to save

        Returns:
            True if successful
        """
        return self._write_json(self.versions_file, data)

    # ==================== Version Config ====================

    def load_version_config(self, tag: str) -> Optional[VersionConfig]:
        """
        Load version-specific config

        Args:
            tag: Version tag (e.g., "v0.2.0")

        Returns:
            VersionConfig or None if not found
        """
        config_file = self.version_configs_dir / f"{tag}-config.json"
        if not config_file.exists():
            return None
        return cast(VersionConfig, self._read_json(config_file))

    def save_version_config(self, tag: str, data: VersionConfig) -> bool:
        """
        Save version-specific config

        Args:
            tag: Version tag (e.g., "v0.2.0")
            data: VersionConfig to save

        Returns:
            True if successful
        """
        config_file = self.version_configs_dir / f"{tag}-config.json"
        return self._write_json(config_file, data)

    def delete_version_config(self, tag: str) -> bool:
        """
        Delete version-specific config

        Args:
            tag: Version tag

        Returns:
            True if deleted, False if didn't exist
        """
        config_file = self.version_configs_dir / f"{tag}-config.json"
        if config_file.exists():
            config_file.unlink()
            return True
        return False

    # ==================== Models Metadata ====================

    def load_models(self) -> ModelsMetadata:
        """
        Load models.json

        Returns:
            ModelsMetadata with model information
        """
        return cast(ModelsMetadata, self._read_json(self.models_file, {}))

    def save_models(self, data: ModelsMetadata) -> bool:
        """
        Save models.json

        Args:
            data: ModelsMetadata to save

        Returns:
            True if successful
        """
        return self._write_json(self.models_file, data)

    # ==================== Custom Nodes Metadata ====================

    def load_custom_nodes(self) -> CustomNodesMetadata:
        """
        Load custom_nodes.json

        Returns:
            CustomNodesMetadata with custom node information
        """
        return cast(CustomNodesMetadata, self._read_json(self.custom_nodes_file, {}))

    def save_custom_nodes(self, data: CustomNodesMetadata) -> bool:
        """
        Save custom_nodes.json

        Args:
            data: CustomNodesMetadata to save

        Returns:
            True if successful
        """
        return self._write_json(self.custom_nodes_file, data)

    # ==================== Workflows Metadata ====================

    def load_workflows(self) -> WorkflowsMetadata:
        """
        Load workflows.json

        Returns:
            WorkflowsMetadata with workflow information
        """
        return cast(WorkflowsMetadata, self._read_json(self.workflows_file, {}))

    def save_workflows(self, data: WorkflowsMetadata) -> bool:
        """
        Save workflows.json

        Args:
            data: WorkflowsMetadata to save

        Returns:
            True if successful
        """
        return self._write_json(self.workflows_file, data)

    # ==================== GitHub Cache ====================

    def load_github_cache(self) -> Optional[GitHubReleasesCache]:
        """
        Load github-releases.json cache

        Returns:
            GitHubReleasesCache or None if not cached
        """
        if not self.github_cache_file.exists():
            return None
        return cast(GitHubReleasesCache, self._read_json(self.github_cache_file))

    def save_github_cache(self, data: GitHubReleasesCache) -> bool:
        """
        Save github-releases.json cache

        Args:
            data: GitHubReleasesCache to save

        Returns:
            True if successful
        """
        return self._write_json(self.github_cache_file, data)

    # ==================== Utility Methods ====================

    def get_all_version_tags(self) -> list[str]:
        """
        Get list of all installed version tags

        Returns:
            List of version tags
        """
        versions = self.load_versions()
        return list(versions.get("installed", {}).keys())

    def version_exists(self, tag: str) -> bool:
        """
        Check if a version is installed

        Args:
            tag: Version tag

        Returns:
            True if version is installed
        """
        versions = self.load_versions()
        return tag in versions.get("installed", {})

    def get_active_version(self) -> Optional[str]:
        """
        Get the currently active/selected version

        Returns:
            Version tag or None
        """
        versions = self.load_versions()
        return versions.get("lastSelectedVersion")

    def set_active_version(self, tag: str) -> bool:
        """
        Set the active version

        Args:
            tag: Version tag to set as active

        Returns:
            True if successful
        """
        versions = self.load_versions()
        if tag not in versions.get("installed", {}):
            return False
        versions["lastSelectedVersion"] = tag
        return self.save_versions(versions)
