#!/usr/bin/env python3
"""
Resource Manager for ComfyUI Version Manager
Main coordinator for shared storage, symlinks, custom nodes, and resource management
"""

from pathlib import Path
from typing import Dict, List, Optional, Tuple

from backend.logging_config import get_logger
from backend.metadata_manager import MetadataManager
from backend.models import ModelsMetadata, RepairReport, ScanResult
from backend.resources.custom_nodes_manager import CustomNodesManager
from backend.resources.model_manager import ModelManager
from backend.resources.shared_storage import SharedStorageManager
from backend.resources.symlink_manager import SymlinkManager

logger = get_logger(__name__)


class ResourceManager:
    """Manages shared resources, symlinks, and custom nodes"""

    def __init__(self, launcher_root: Path, metadata_manager: MetadataManager):
        """
        Initialize resource manager

        Args:
            launcher_root: Path to launcher root directory
            metadata_manager: MetadataManager instance for persistence
        """
        self.launcher_root = Path(launcher_root)
        self.metadata_manager = metadata_manager

        # Main directories
        self.shared_dir = self.launcher_root / "shared-resources"
        self.versions_dir = self.launcher_root / "comfyui-versions"

        # Shared resource subdirectories
        self.shared_models_dir = self.shared_dir / "models"
        self.shared_custom_nodes_cache_dir = self.shared_dir / "custom_nodes_cache"
        self.shared_user_dir = self.shared_dir / "user"
        self.shared_workflows_dir = self.shared_user_dir / "workflows"
        self.shared_settings_dir = self.shared_user_dir / "settings"

        # Initialize specialized managers
        self.storage_mgr = SharedStorageManager(
            self.shared_dir, self.versions_dir, self.launcher_root
        )

        self.symlink_mgr = SymlinkManager(
            self.shared_models_dir, self.shared_user_dir, self.versions_dir, self.launcher_root
        )

        self.model_mgr = ModelManager(self.shared_models_dir, self.metadata_manager)

        self.custom_nodes_mgr = CustomNodesManager(
            self.shared_custom_nodes_cache_dir, self.versions_dir
        )

    # ==================== Shared Storage Operations ====================

    def initialize_shared_storage(self) -> bool:
        """
        Create shared-resources directory structure

        Returns:
            True if successful
        """
        return self.storage_mgr.initialize_shared_storage()

    def discover_model_directories(self, comfyui_path: Path) -> List[str]:
        """
        Discover model directories from ComfyUI installation
        Parses folder_paths.py to find model directory names

        Args:
            comfyui_path: Path to ComfyUI installation

        Returns:
            List of model directory names (e.g., ['checkpoints', 'loras', 'vae'])
        """
        return self.storage_mgr.discover_model_directories(comfyui_path)

    def sync_shared_model_structure(self, comfyui_version_path: Path) -> bool:
        """
        Ensure shared models has all directories from this ComfyUI version
        NEVER removes directories (preserve models for other versions)

        Args:
            comfyui_version_path: Path to ComfyUI version installation

        Returns:
            True if successful
        """
        return self.storage_mgr.sync_shared_model_structure(comfyui_version_path)

    def migrate_existing_files(
        self, version_path: Path, auto_merge: bool = False
    ) -> Tuple[int, int, List[str]]:
        """
        Scan version directory for real files and move to shared storage

        Args:
            version_path: Path to version directory
            auto_merge: If True, automatically merge files (skip conflicts)

        Returns:
            Tuple of (files_moved, conflicts, conflict_paths)
        """
        return self.storage_mgr.migrate_existing_files(version_path, auto_merge)

    def scan_shared_storage(self) -> ScanResult:
        """
        Scan shared storage and update metadata

        Returns:
            ScanResult with counts and total size
        """
        return self.storage_mgr.scan_shared_storage()

    # ==================== Symlink Operations ====================

    def setup_version_symlinks(self, version_tag: str) -> bool:
        """
        Setup all symlinks for a version
        - Symlinks models directory
        - Symlinks user data (workflows, settings)
        - Does NOT symlink custom_nodes (real files per version)

        Args:
            version_tag: Version tag (e.g., "v0.2.0")

        Returns:
            True if successful
        """
        return self.symlink_mgr.setup_version_symlinks(version_tag)

    def validate_and_repair_symlinks(self, version_tag: str) -> RepairReport:
        """
        Check for broken symlinks and attempt repair

        Args:
            version_tag: Version tag to check

        Returns:
            RepairReport with broken, repaired, and removed symlinks
        """
        return self.symlink_mgr.validate_and_repair_symlinks(version_tag)

    # ==================== Model Operations ====================

    def get_models(self) -> ModelsMetadata:
        """
        Get all models from shared storage

        Returns:
            Dict mapping model paths to model info
        """
        return self.model_mgr.get_models()

    def add_model(self, source_path: Path, category: str, update_metadata: bool = True) -> bool:
        """
        Add a model to shared storage

        Args:
            source_path: Path to model file
            category: Model category (e.g., "checkpoints", "loras")
            update_metadata: Whether to update models.json

        Returns:
            True if successful
        """
        return self.model_mgr.add_model(source_path, category, update_metadata)

    def remove_model(self, model_path: str) -> bool:
        """
        Remove a model from shared storage

        Args:
            model_path: Relative path to model (e.g., "checkpoints/model.safetensors")

        Returns:
            True if successful
        """
        return self.model_mgr.remove_model(model_path)

    # ==================== Custom Nodes Operations ====================

    def get_version_custom_nodes_dir(self, version_tag: str) -> Path:
        """
        Get the custom_nodes directory for a specific version

        Args:
            version_tag: Version tag

        Returns:
            Path to version's custom_nodes directory
        """
        return self.custom_nodes_mgr.get_version_custom_nodes_dir(version_tag)

    def list_version_custom_nodes(self, version_tag: str) -> List[str]:
        """
        List custom nodes installed for a specific version

        Args:
            version_tag: Version tag

        Returns:
            List of custom node directory names
        """
        return self.custom_nodes_mgr.list_version_custom_nodes(version_tag)

    def install_custom_node(
        self, git_url: str, version_tag: str, node_name: Optional[str] = None
    ) -> bool:
        """
        Install a custom node for a specific ComfyUI version
        Creates a real copy (not symlink) in the version's custom_nodes directory

        Args:
            git_url: Git repository URL
            version_tag: ComfyUI version tag
            node_name: Optional custom node name (extracted from URL if not provided)

        Returns:
            True if successful
        """
        return self.custom_nodes_mgr.install_custom_node(git_url, version_tag, node_name)

    def update_custom_node(self, node_name: str, version_tag: str) -> bool:
        """
        Update a custom node to latest version

        Args:
            node_name: Custom node directory name
            version_tag: ComfyUI version tag

        Returns:
            True if successful
        """
        return self.custom_nodes_mgr.update_custom_node(node_name, version_tag)

    def remove_custom_node(self, node_name: str, version_tag: str) -> bool:
        """
        Remove a custom node from a specific ComfyUI version

        Args:
            node_name: Custom node directory name
            version_tag: ComfyUI version tag

        Returns:
            True if successful
        """
        return self.custom_nodes_mgr.remove_custom_node(node_name, version_tag)

    def cache_custom_node_repo(self, git_url: str) -> Optional[Path]:
        """
        Clone or update a custom node repository in the cache
        Creates a bare git repo for efficient storage

        Args:
            git_url: Git repository URL

        Returns:
            Path to cached repo or None on failure
        """
        return self.custom_nodes_mgr.cache_custom_node_repo(git_url)


if __name__ == "__main__":
    # For testing - demonstrate resource manager
    from backend.utils import get_launcher_root

    launcher_root = get_launcher_root()
    launcher_data_dir = launcher_root / "launcher-data"

    # Initialize metadata manager
    metadata_mgr = MetadataManager(launcher_data_dir)

    # Initialize resource manager
    resource_mgr = ResourceManager(launcher_root, metadata_mgr)

    logger.info("=== Resource Manager Test ===\n")

    # Initialize shared storage
    logger.info("Initializing shared storage...")
    if resource_mgr.initialize_shared_storage():
        logger.info("✓ Shared storage initialized\n")
    else:
        logger.error("✗ Failed to initialize shared storage\n")

    # Scan shared storage
    logger.info("Scanning shared storage...")
    scan_result = resource_mgr.scan_shared_storage()
    logger.info(f"✓ Scan complete:")
    logger.info(f"  Models: {scan_result['modelsFound']}")
    logger.info(f"  Workflows: {scan_result['workflowsFound']}")
    logger.info(f"  Total size: {scan_result['totalSize']:,} bytes\n")

    # Check if we have any installed versions
    versions = metadata_mgr.load_versions()
    if versions.get("installed"):
        logger.info("Testing symlink setup for installed versions:")
        for version_tag in versions["installed"].keys():
            logger.info(f"\nVersion: {version_tag}")

            # Setup symlinks
            if resource_mgr.setup_version_symlinks(version_tag):
                logger.info(f"  ✓ Symlinks created")
            else:
                logger.error(f"  ✗ Failed to create symlinks")

            # Validate symlinks
            repair_report = resource_mgr.validate_and_repair_symlinks(version_tag)
            logger.info(
                f"  Validation: {len(repair_report['broken'])} broken, "
                f"{len(repair_report['repaired'])} repaired, "
                f"{len(repair_report['removed'])} removed"
            )
    else:
        logger.info("No versions installed yet")
