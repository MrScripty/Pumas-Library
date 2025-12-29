#!/usr/bin/env python3
"""
Model Manager
Handles model file operations and metadata
"""

import shutil
from pathlib import Path
from typing import Dict

from backend.logging_config import get_logger
from backend.metadata_manager import MetadataManager
from backend.models import ModelInfo, get_iso_timestamp
from backend.utils import calculate_file_hash, ensure_directory

logger = get_logger(__name__)


class ModelManager:
    """Manages model files in shared storage"""

    def __init__(self, shared_models_dir: Path, metadata_manager: MetadataManager):
        """
        Initialize model manager

        Args:
            shared_models_dir: Path to shared models directory
            metadata_manager: MetadataManager instance
        """
        self.shared_models_dir = Path(shared_models_dir)
        self.metadata_manager = metadata_manager

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
