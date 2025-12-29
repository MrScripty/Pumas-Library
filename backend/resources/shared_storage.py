#!/usr/bin/env python3
"""
Shared Storage Manager
Handles shared storage initialization and model directory discovery
"""

import re
import shutil
from pathlib import Path
from typing import List, Tuple

from backend.logging_config import get_logger
from backend.models import ScanResult
from backend.utils import ensure_directory

logger = get_logger(__name__)


class SharedStorageManager:
    """Manages shared storage structure and model directories"""

    def __init__(self, shared_dir: Path, versions_dir: Path, launcher_root: Path):
        """
        Initialize shared storage manager

        Args:
            shared_dir: Path to shared-resources directory
            versions_dir: Path to comfyui-versions directory
            launcher_root: Path to launcher root directory
        """
        self.shared_dir = Path(shared_dir)
        self.versions_dir = Path(versions_dir)
        self.launcher_root = Path(launcher_root)

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
            pattern = r'folder_names_and_paths\["([^"]+)"\]'
            matches = re.findall(pattern, content)

            if matches:
                model_dirs = list(set(matches))
                logger.info(f"Discovered {len(model_dirs)} model directories from folder_paths.py")
                return sorted(model_dirs)
            else:
                logger.warning("Could not parse folder_paths.py, using defaults")
                return self._get_default_model_directories()

        except (IOError, OSError, UnicodeDecodeError) as e:
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
                            logger.warning(
                                f"Conflict: {model_file.name} already exists in shared storage"
                            )
                            continue

                    # Move file to shared storage
                    try:
                        shutil.move(str(model_file), str(shared_file_path))
                        files_moved += 1
                        logger.info(f"Moved: {category_name}/{model_file.name} -> shared storage")
                    except (IOError, OSError, PermissionError) as e:
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
                            logger.warning(
                                f"Conflict: workflow {workflow_file.name} already exists"
                            )
                            continue

                    try:
                        shutil.move(str(workflow_file), str(shared_workflow_path))
                        files_moved += 1
                        logger.info(f"Moved: workflow {workflow_file.name} -> shared storage")
                    except (IOError, OSError, PermissionError) as e:
                        logger.error(f"Error moving workflow {workflow_file}: {e}", exc_info=True)

            # Remove empty directories
            if workflows_dir.exists() and not list(workflows_dir.iterdir()):
                workflows_dir.rmdir()
            if not list(user_dir.iterdir()):
                user_dir.rmdir()

        return (files_moved, conflicts, conflict_paths)

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
