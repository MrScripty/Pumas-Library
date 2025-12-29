#!/usr/bin/env python3
"""
Directory Structure Setup
Creates the necessary directories for the version manager
"""

from pathlib import Path
from typing import List

from backend.logging_config import get_logger

logger = get_logger(__name__)


class DirectorySetup:
    """Handles creation and initialization of directory structure"""

    def __init__(self, launcher_root: Path):
        """
        Initialize directory setup

        Args:
            launcher_root: Path to launcher root directory
        """
        self.launcher_root = Path(launcher_root)

        # Define main directories
        self.comfyui_versions_dir = self.launcher_root / "comfyui-versions"
        self.shared_resources_dir = self.launcher_root / "shared-resources"
        self.launcher_data_dir = self.launcher_root / "launcher-data"

        # Shared resources subdirectories
        self.shared_models_dir = self.shared_resources_dir / "models"
        self.shared_custom_nodes_cache_dir = self.shared_resources_dir / "custom_nodes_cache"
        self.shared_user_dir = self.shared_resources_dir / "user"

        # Launcher data subdirectories
        self.metadata_dir = self.launcher_data_dir / "metadata"
        self.config_dir = self.launcher_data_dir / "config"
        self.cache_dir = self.launcher_data_dir / "cache"
        self.version_configs_dir = self.config_dir / "version-configs"

    def create_all_directories(self) -> bool:
        """
        Create all required directories

        Returns:
            True if all directories were created successfully
        """
        directories = [
            # Main directories
            self.comfyui_versions_dir,
            self.shared_resources_dir,
            self.launcher_data_dir,
            # Shared resources
            self.shared_models_dir,
            self.shared_custom_nodes_cache_dir,
            self.shared_user_dir,
            self.shared_user_dir / "workflows",
            self.shared_user_dir / "settings",
            # Launcher data
            self.metadata_dir,
            self.config_dir,
            self.cache_dir,
            self.version_configs_dir,
        ]

        success = True
        for directory in directories:
            try:
                directory.mkdir(parents=True, exist_ok=True)
                logger.info(f"Created/verified: {directory.relative_to(self.launcher_root)}")
            except OSError as e:
                logger.error(f"Error creating {directory}: {e}", exc_info=True)
                success = False

        return success

    def initialize_model_directories(self) -> bool:
        """
        Create model category subdirectories
        Based on known ComfyUI model categories

        Returns:
            True if successful
        """
        # Known ComfyUI model directories (from plan appendix)
        model_categories = [
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

        success = True
        for category in model_categories:
            try:
                category_dir = self.shared_models_dir / category
                category_dir.mkdir(parents=True, exist_ok=True)
            except OSError as e:
                logger.error(f"Error creating model category {category}: {e}", exc_info=True)
                success = False

        return success

    def verify_structure(self) -> dict[str, bool]:
        """
        Verify that all required directories exist

        Returns:
            Dict mapping directory names to existence status
        """
        checks = {
            "comfyui-versions": self.comfyui_versions_dir.exists(),
            "shared-resources": self.shared_resources_dir.exists(),
            "launcher-data": self.launcher_data_dir.exists(),
            "shared-resources/models": self.shared_models_dir.exists(),
            "shared-resources/custom_nodes_cache": self.shared_custom_nodes_cache_dir.exists(),
            "shared-resources/user": self.shared_user_dir.exists(),
            "launcher-data/metadata": self.metadata_dir.exists(),
            "launcher-data/config": self.config_dir.exists(),
            "launcher-data/cache": self.cache_dir.exists(),
            "launcher-data/config/version-configs": self.version_configs_dir.exists(),
        }
        return checks

    def get_structure_summary(self) -> str:
        """
        Get a text summary of the directory structure

        Returns:
            String representation of directory tree
        """
        summary = []
        summary.append(f"Launcher Root: {self.launcher_root}")
        summary.append("")
        summary.append("Directory Structure:")
        summary.append(f"├── comfyui-versions/")
        summary.append(f"├── shared-resources/")
        summary.append(f"│   ├── models/")
        summary.append(f"│   │   ├── checkpoints/")
        summary.append(f"│   │   ├── loras/")
        summary.append(f"│   │   ├── vae/")
        summary.append(f"│   │   └── ... (other model categories)")
        summary.append(f"│   ├── custom_nodes_cache/")
        summary.append(f"│   └── user/")
        summary.append(f"│       ├── workflows/")
        summary.append(f"│       └── settings/")
        summary.append(f"└── launcher-data/")
        summary.append(f"    ├── metadata/")
        summary.append(f"    ├── config/")
        summary.append(f"    │   └── version-configs/")
        summary.append(f"    └── cache/")
        summary.append("")

        # Add verification status
        verification = self.verify_structure()
        all_ok = all(verification.values())

        if all_ok:
            summary.append("Status: All directories present ✓")
        else:
            summary.append("Status: Some directories missing ✗")
            for name, exists in verification.items():
                if not exists:
                    summary.append(f"  Missing: {name}")

        return "\n".join(summary)


def initialize_directories(launcher_root: Path) -> bool:
    """
    Convenience function to initialize all directories

    Args:
        launcher_root: Path to launcher root

    Returns:
        True if successful
    """
    setup = DirectorySetup(launcher_root)
    success = setup.create_all_directories()
    if success:
        success = setup.initialize_model_directories()
    return success


if __name__ == "__main__":
    # For testing - run from launcher root
    import sys
    from pathlib import Path

    if len(sys.argv) > 1:
        launcher_root = Path(sys.argv[1])
    else:
        # Use parent of backend/ as launcher root
        launcher_root = Path(__file__).parent.parent

    setup = DirectorySetup(launcher_root)
    logger.info(setup.get_structure_summary())
    logger.info("\nInitializing directories...")

    if setup.create_all_directories():
        logger.info("✓ Main directories created")
    else:
        logger.error("✗ Error creating main directories")
        sys.exit(1)

    if setup.initialize_model_directories():
        logger.info("✓ Model directories created")
    else:
        logger.error("✗ Error creating model directories")
        sys.exit(1)

    logger.info("\n" + setup.get_structure_summary())
