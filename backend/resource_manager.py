#!/usr/bin/env python3
"""
Resource Manager for ComfyUI Version Manager
Handles shared storage, symlinks, custom nodes, and resource management
"""

import json
import shutil
from pathlib import Path
from typing import Dict, List, Optional, Tuple

from backend.logging_config import get_logger
from backend.metadata_manager import MetadataManager
from backend.models import ModelInfo, RepairReport, ScanResult, get_iso_timestamp
from backend.utils import (
    calculate_file_hash,
    ensure_directory,
    get_directory_size,
    is_broken_symlink,
    is_valid_symlink,
    make_relative_symlink,
)

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

    def initialize_shared_storage(self) -> bool:
        """
        Create shared-resources directory structure

        Returns:
            True if successful
        """
        directories = [
            self.shared_dir,
            self.shared_models_dir,
            self.shared_custom_nodes_cache_dir,
            self.shared_user_dir,
            self.shared_workflows_dir,
            self.shared_settings_dir,
        ]

        success = True
        for directory in directories:
            if not ensure_directory(directory):
                success = False

        return success

    def discover_model_directories(self, comfyui_path: Path) -> List[str]:
        """
        Discover model directories from ComfyUI installation
        Parses folder_paths.py to find model directory names

        Args:
            comfyui_path: Path to ComfyUI installation

        Returns:
            List of model directory names (e.g., ['checkpoints', 'loras', 'vae'])
        """
        folder_paths_file = comfyui_path / "comfy" / "folder_paths.py"

        if not folder_paths_file.exists():
            logger.warning(f"Warning: folder_paths.py not found in {comfyui_path}")
            return self._get_default_model_directories()

        try:
            # Read and parse folder_paths.py to find model directories
            with open(folder_paths_file, "r") as f:
                content = f.read()

            # Look for folder_names_and_paths dictionary
            # This is a simple extraction, might need refinement
            model_dirs = []

            # Common pattern: folder_names_and_paths["checkpoints"] = ...
            import re

            pattern = r'folder_names_and_paths\["([^"]+)"\]'
            matches = re.findall(pattern, content)

            if matches:
                model_dirs = list(set(matches))
                logger.info(f"Discovered {len(model_dirs)} model directories from folder_paths.py")
                return sorted(model_dirs)
            else:
                logger.info("Could not parse folder_paths.py, using defaults")
                return self._get_default_model_directories()

        except Exception as e:
            logger.error(f"Error parsing folder_paths.py: {e}", exc_info=True)
            return self._get_default_model_directories()

    def _get_default_model_directories(self) -> List[str]:
        """
        Get default model directories based on known ComfyUI structure

        Returns:
            List of default model directory names
        """
        return [
            "checkpoints",
            "clip",
            "clip_vision",
            "configs",
            "controlnet",
            "diffusion_models",
            "embeddings",
            "loras",
            "photomaker",
            "style_models",
            "unet",
            "upscale_models",
            "vae",
            "vae_approx",
        ]

    def sync_shared_model_structure(self, comfyui_version_path: Path) -> bool:
        """
        Ensure shared models has all directories from this ComfyUI version
        NEVER removes directories (preserve models for other versions)

        Args:
            comfyui_version_path: Path to ComfyUI version installation

        Returns:
            True if successful
        """
        # Discover model directories from this version
        model_dirs = self.discover_model_directories(comfyui_version_path)

        success = True
        created_count = 0

        for model_dir in model_dirs:
            target_dir = self.shared_models_dir / model_dir
            if not target_dir.exists():
                if ensure_directory(target_dir):
                    created_count += 1
                    logger.info(f"Created model directory: {model_dir}")
                else:
                    success = False

        if created_count > 0:
            logger.info(f"Created {created_count} new model directories")

        return success

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
        version_path = self.versions_dir / version_tag

        if not version_path.exists():
            logger.error(f"Error: Version directory not found: {version_path}")
            return False

        success = True

        # 1. Symlink models directory
        models_link = version_path / "models"
        if not make_relative_symlink(self.shared_models_dir, models_link):
            logger.error(f"Failed to create models symlink for {version_tag}")
            success = False
        else:
            logger.info(f"Created models symlink: {version_tag}/models -> shared-resources/models")

        # 2. Symlink user directory
        user_link = version_path / "user"
        if not make_relative_symlink(self.shared_user_dir, user_link):
            logger.error(f"Failed to create user symlink for {version_tag}")
            success = False
        else:
            logger.info(f"Created user symlink: {version_tag}/user -> shared-resources/user")

        return success

    def validate_and_repair_symlinks(self, version_tag: str) -> RepairReport:
        """
        Check for broken symlinks and attempt repair

        Args:
            version_tag: Version tag to check

        Returns:
            RepairReport with broken, repaired, and removed symlinks
        """
        version_path = self.versions_dir / version_tag

        report: RepairReport = {"broken": [], "repaired": [], "removed": []}

        if not version_path.exists():
            logger.error(f"Error: Version directory not found: {version_path}")
            return report

        # Check key symlinks
        symlinks_to_check = {
            "models": self.shared_models_dir,
            "user": self.shared_user_dir,
        }

        for link_name, expected_target in symlinks_to_check.items():
            link_path = version_path / link_name

            if is_broken_symlink(link_path):
                report["broken"].append(str(link_path.relative_to(self.launcher_root)))

                # Try to repair
                if expected_target.exists():
                    if make_relative_symlink(expected_target, link_path):
                        report["repaired"].append(str(link_path.relative_to(self.launcher_root)))
                        logger.info(f"Repaired symlink: {link_name}")
                    else:
                        logger.error(f"Failed to repair symlink: {link_name}")
                else:
                    # Target doesn't exist, remove broken symlink
                    link_path.unlink()
                    report["removed"].append(str(link_path.relative_to(self.launcher_root)))
                    logger.info(f"Removed broken symlink: {link_name}")

            elif not link_path.exists():
                # Symlink doesn't exist, create it
                if expected_target.exists():
                    if make_relative_symlink(expected_target, link_path):
                        report["repaired"].append(str(link_path.relative_to(self.launcher_root)))
                        logger.info(f"Created missing symlink: {link_name}")

        return report

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
        files_moved = 0
        conflicts = 0
        conflict_paths = []

        # Check for real models directory
        models_dir = version_path / "models"
        if models_dir.exists() and not models_dir.is_symlink():
            logger.info(f"Found real models directory in {version_path.name}")

            # Scan for model files
            for category_dir in models_dir.iterdir():
                if not category_dir.is_dir():
                    continue

                category_name = category_dir.name
                shared_category_dir = self.shared_models_dir / category_name

                # Ensure shared category exists
                ensure_directory(shared_category_dir)

                # Move model files
                for model_file in category_dir.iterdir():
                    if not model_file.is_file():
                        continue

                    shared_file_path = shared_category_dir / model_file.name

                    if shared_file_path.exists():
                        # Conflict: file already exists in shared storage
                        conflicts += 1
                        conflict_paths.append(str(model_file.relative_to(self.launcher_root)))

                        if not auto_merge:
                            logger.info(
                                f"Conflict: {model_file.name} already exists in shared storage"
                            )
                            continue

                    # Move file to shared storage
                    try:
                        shutil.move(str(model_file), str(shared_file_path))
                        files_moved += 1
                        logger.info(f"Moved: {category_name}/{model_file.name} -> shared storage")
                    except Exception as e:
                        logger.error(f"Error moving {model_file}: {e}", exc_info=True)

            # Remove empty category directories
            for category_dir in models_dir.iterdir():
                if category_dir.is_dir() and not list(category_dir.iterdir()):
                    category_dir.rmdir()

            # Remove models directory if empty
            if not list(models_dir.iterdir()):
                models_dir.rmdir()

        # Check for real user directory
        user_dir = version_path / "user"
        if user_dir.exists() and not user_dir.is_symlink():
            logger.info(f"Found real user directory in {version_path.name}")

            # Migrate workflows
            workflows_dir = user_dir / "workflows"
            if workflows_dir.exists():
                ensure_directory(self.shared_workflows_dir)

                for workflow_file in workflows_dir.iterdir():
                    if not workflow_file.is_file():
                        continue

                    shared_workflow_path = self.shared_workflows_dir / workflow_file.name

                    if shared_workflow_path.exists():
                        conflicts += 1
                        conflict_paths.append(str(workflow_file.relative_to(self.launcher_root)))

                        if not auto_merge:
                            logger.info(f"Conflict: workflow {workflow_file.name} already exists")
                            continue

                    try:
                        shutil.move(str(workflow_file), str(shared_workflow_path))
                        files_moved += 1
                        logger.info(f"Moved: workflow {workflow_file.name} -> shared storage")
                    except Exception as e:
                        logger.error(f"Error moving workflow {workflow_file}: {e}", exc_info=True)

            # Remove empty directories
            if workflows_dir.exists() and not list(workflows_dir.iterdir()):
                workflows_dir.rmdir()
            if not list(user_dir.iterdir()):
                user_dir.rmdir()

        return (files_moved, conflicts, conflict_paths)

    def get_models(self) -> dict:
        """
        Get all models from shared storage

        Returns:
            Dict mapping model paths to model info
        """
        return self.metadata_manager.load_models()

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
        if not source_path.exists():
            logger.error(f"Error: Source file not found: {source_path}")
            return False

        # Ensure category directory exists
        category_dir = self.shared_models_dir / category
        ensure_directory(category_dir)

        # Destination path
        dest_path = category_dir / source_path.name

        if dest_path.exists():
            logger.error(f"Error: Model already exists: {dest_path.name}")
            return False

        try:
            # Copy file
            shutil.copy2(str(source_path), str(dest_path))
            logger.info(f"Added model: {category}/{source_path.name}")

            # Update metadata if requested
            if update_metadata:
                self._update_model_metadata(dest_path, category)

            return True

        except Exception as e:
            logger.error(f"Error adding model: {e}", exc_info=True)
            return False

    def _update_model_metadata(self, model_path: Path, category: str):
        """
        Update models.json with model information

        Args:
            model_path: Path to model file
            category: Model category
        """
        try:
            # Load current metadata
            metadata = self.metadata_manager.load_models()

            # Calculate file hash
            file_hash = calculate_file_hash(model_path)

            # Create model info
            relative_path = str(model_path.relative_to(self.shared_models_dir))

            model_info: ModelInfo = {
                "path": relative_path,
                "size": model_path.stat().st_size,
                "sha256": file_hash or "",
                "addedDate": get_iso_timestamp(),
                "lastUsed": get_iso_timestamp(),
                "tags": [],
                "modelType": category,
                "usedByVersions": [],
                "source": "manual",
            }

            # Add to metadata
            metadata[relative_path] = model_info

            # Save metadata
            self.metadata_manager.save_models(metadata)
            logger.info(f"Updated metadata for {model_path.name}")

        except Exception as e:
            logger.error(f"Error updating model metadata: {e}", exc_info=True)

    def remove_model(self, model_path: str) -> bool:
        """
        Remove a model from shared storage

        Args:
            model_path: Relative path to model (e.g., "checkpoints/model.safetensors")

        Returns:
            True if successful
        """
        full_path = self.shared_models_dir / model_path

        if not full_path.exists():
            logger.error(f"Error: Model not found: {model_path}")
            return False

        try:
            # Remove file
            full_path.unlink()
            logger.info(f"Removed model: {model_path}")

            # Update metadata
            metadata = self.metadata_manager.load_models()
            if model_path in metadata:
                del metadata[model_path]
                self.metadata_manager.save_models(metadata)

            return True

        except Exception as e:
            logger.error(f"Error removing model: {e}", exc_info=True)
            return False

    def scan_shared_storage(self) -> ScanResult:
        """
        Scan shared storage and update metadata

        Returns:
            ScanResult with counts and total size
        """
        models_found = 0
        workflows_found = 0
        total_size = 0

        # Scan models
        if self.shared_models_dir.exists():
            for category_dir in self.shared_models_dir.iterdir():
                if not category_dir.is_dir():
                    continue

                for model_file in category_dir.iterdir():
                    if model_file.is_file():
                        models_found += 1
                        total_size += model_file.stat().st_size

        # Scan workflows
        if self.shared_workflows_dir.exists():
            for workflow_file in self.shared_workflows_dir.iterdir():
                if workflow_file.is_file():
                    workflows_found += 1
                    total_size += workflow_file.stat().st_size

        result: ScanResult = {
            "modelsFound": models_found,
            "workflowsFound": workflows_found,
            "customNodesFound": 0,  # Custom nodes not in shared storage
            "totalSize": total_size,
        }

        return result

    def get_version_custom_nodes_dir(self, version_tag: str) -> Path:
        """
        Get the custom_nodes directory for a specific version

        Args:
            version_tag: Version tag

        Returns:
            Path to version's custom_nodes directory
        """
        return self.versions_dir / version_tag / "custom_nodes"

    def list_version_custom_nodes(self, version_tag: str) -> List[str]:
        """
        List custom nodes installed for a specific version

        Args:
            version_tag: Version tag

        Returns:
            List of custom node directory names
        """
        custom_nodes_dir = self.get_version_custom_nodes_dir(version_tag)

        if not custom_nodes_dir.exists():
            return []

        try:
            return [
                d.name
                for d in custom_nodes_dir.iterdir()
                if d.is_dir() and not d.name.startswith(".")
            ]
        except Exception as e:
            logger.error(f"Error listing custom nodes: {e}", exc_info=True)
            return []

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
        from backend.utils import run_command

        # Extract node name from git URL if not provided
        if node_name is None:
            # Extract from URL like: https://github.com/user/ComfyUI-CustomNode.git
            node_name = git_url.rstrip("/").split("/")[-1]
            if node_name.endswith(".git"):
                node_name = node_name[:-4]

        # Get custom nodes directory for this version
        custom_nodes_dir = self.get_version_custom_nodes_dir(version_tag)
        ensure_directory(custom_nodes_dir)

        node_install_path = custom_nodes_dir / node_name

        if node_install_path.exists():
            logger.info(f"Custom node already installed: {node_name}")
            return False

        # Clone to version's custom_nodes directory
        logger.info(f"Installing custom node {node_name} for {version_tag}...")

        success, stdout, stderr = run_command(
            ["git", "clone", git_url, str(node_install_path)],
            timeout=300,  # 5 minute timeout for large repos
        )

        if not success:
            logger.error(f"Error cloning custom node: {stderr}")
            return False

        logger.info(f"✓ Installed custom node: {node_name}")

        # Check for requirements.txt and warn user
        requirements_file = node_install_path / "requirements.txt"
        if requirements_file.exists():
            logger.info(f"  Note: {node_name} has requirements.txt")
            logger.info(f"  You may need to install dependencies for {version_tag}")

        return True

    def update_custom_node(self, node_name: str, version_tag: str) -> bool:
        """
        Update a custom node to latest version

        Args:
            node_name: Custom node directory name
            version_tag: ComfyUI version tag

        Returns:
            True if successful
        """
        from backend.utils import run_command

        node_path = self.get_version_custom_nodes_dir(version_tag) / node_name

        if not node_path.exists():
            logger.error(f"Custom node not found: {node_name}")
            return False

        # Check if it's a git repository
        if not (node_path / ".git").exists():
            logger.error(f"Not a git repository: {node_name}")
            return False

        logger.info(f"Updating custom node {node_name}...")

        # Git pull
        success, stdout, stderr = run_command(["git", "pull"], cwd=node_path, timeout=60)

        if not success:
            logger.error(f"Error updating custom node: {stderr}")
            return False

        logger.info(f"✓ Updated custom node: {node_name}")
        logger.info(stdout)

        # Check if requirements changed
        requirements_file = node_path / "requirements.txt"
        if requirements_file.exists():
            logger.info(f"  Note: Check if requirements.txt changed")

        return True

    def remove_custom_node(self, node_name: str, version_tag: str) -> bool:
        """
        Remove a custom node from a specific ComfyUI version

        Args:
            node_name: Custom node directory name
            version_tag: ComfyUI version tag

        Returns:
            True if successful
        """
        node_path = self.get_version_custom_nodes_dir(version_tag) / node_name

        if not node_path.exists():
            logger.error(f"Custom node not found: {node_name}")
            return False

        try:
            shutil.rmtree(node_path)
            logger.info(f"✓ Removed custom node: {node_name} from {version_tag}")
            return True
        except Exception as e:
            logger.error(f"Error removing custom node: {e}", exc_info=True)
            return False

    def cache_custom_node_repo(self, git_url: str) -> Optional[Path]:
        """
        Clone or update a custom node repository in the cache
        Creates a bare git repo for efficient storage

        Args:
            git_url: Git repository URL

        Returns:
            Path to cached repo or None on failure
        """
        from backend.utils import run_command

        # Extract repo name from URL
        repo_name = git_url.rstrip("/").split("/")[-1]
        if repo_name.endswith(".git"):
            repo_name = repo_name[:-4]

        cache_path = self.shared_custom_nodes_cache_dir / f"{repo_name}.git"

        if cache_path.exists():
            # Update existing cache
            logger.info(f"Updating cached repo: {repo_name}")
            success, stdout, stderr = run_command(
                ["git", "fetch", "--all"], cwd=cache_path, timeout=60
            )

            if not success:
                logger.warning(f"Warning: Failed to update cache: {stderr}")

            return cache_path
        else:
            # Clone as bare repo
            logger.info(f"Caching custom node repo: {repo_name}")
            ensure_directory(self.shared_custom_nodes_cache_dir)

            success, stdout, stderr = run_command(
                ["git", "clone", "--bare", git_url, str(cache_path)], timeout=300
            )

            if not success:
                logger.error(f"Error caching repo: {stderr}")
                return None

            logger.info(f"✓ Cached repo: {repo_name}")
            return cache_path


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
        logger.info("✗ Failed to initialize shared storage\n")

    # Scan shared storage
    logger.info("Scanning shared storage...")
    scan_result = resource_mgr.scan_shared_storage()
    logger.info(f"✓ Scan complete:")
    logger.info(f"  Models: {scan_result['modelsFound']}")
    logger.info(f"  Workflows: {scan_result['workflowsFound']}")
    logger.info(f"  Total size: {scan_result['totalSize']:,} bytes\n")

    # Check if we have any installed versions
    versions = metadata_mgr.load_versions_metadata()
    if versions.get("installed"):
        logger.info("Testing symlink setup for installed versions:")
        for version_tag in versions["installed"].keys():
            logger.info(f"\nVersion: {version_tag}")

            # Setup symlinks
            if resource_mgr.setup_version_symlinks(version_tag):
                logger.info(f"  ✓ Symlinks created")
            else:
                logger.info(f"  ✗ Failed to create symlinks")

            # Validate symlinks
            repair_report = resource_mgr.validate_and_repair_symlinks(version_tag)
            logger.info(
                f"  Validation: {len(repair_report['broken'])} broken, "
                f"{len(repair_report['repaired'])} repaired, "
                f"{len(repair_report['removed'])} removed"
            )
    else:
        logger.info("No versions installed yet")
