#!/usr/bin/env python3
"""
Custom Nodes Manager
Handles custom node installation, updates, and caching
"""

import shutil
from pathlib import Path
from typing import Optional, List

from backend.utils import run_command, ensure_directory


class CustomNodesManager:
    """Manages custom nodes for ComfyUI versions"""

    def __init__(self, shared_custom_nodes_cache_dir: Path, versions_dir: Path):
        """
        Initialize custom nodes manager

        Args:
            shared_custom_nodes_cache_dir: Path to custom nodes cache directory
            versions_dir: Path to comfyui-versions directory
        """
        self.shared_custom_nodes_cache_dir = Path(shared_custom_nodes_cache_dir)
        self.versions_dir = Path(versions_dir)

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
                d.name for d in custom_nodes_dir.iterdir()
                if d.is_dir() and not d.name.startswith('.')
            ]
        except Exception as e:
            print(f"Error listing custom nodes: {e}")
            return []

    def install_custom_node(
        self,
        git_url: str,
        version_tag: str,
        node_name: Optional[str] = None
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
        # Extract node name from git URL if not provided
        if node_name is None:
            # Extract from URL like: https://github.com/user/ComfyUI-CustomNode.git
            node_name = git_url.rstrip('/').split('/')[-1]
            if node_name.endswith('.git'):
                node_name = node_name[:-4]

        # Get custom nodes directory for this version
        custom_nodes_dir = self.get_version_custom_nodes_dir(version_tag)
        ensure_directory(custom_nodes_dir)

        node_install_path = custom_nodes_dir / node_name

        if node_install_path.exists():
            print(f"Custom node already installed: {node_name}")
            return False

        # Clone to version's custom_nodes directory
        print(f"Installing custom node {node_name} for {version_tag}...")

        success, stdout, stderr = run_command(
            ['git', 'clone', git_url, str(node_install_path)],
            timeout=300  # 5 minute timeout for large repos
        )

        if not success:
            print(f"Error cloning custom node: {stderr}")
            return False

        print(f"✓ Installed custom node: {node_name}")

        # Check for requirements.txt and warn user
        requirements_file = node_install_path / "requirements.txt"
        if requirements_file.exists():
            print(f"  Note: {node_name} has requirements.txt")
            print(f"  You may need to install dependencies for {version_tag}")

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
        node_path = self.get_version_custom_nodes_dir(version_tag) / node_name

        if not node_path.exists():
            print(f"Custom node not found: {node_name}")
            return False

        # Check if it's a git repository
        if not (node_path / ".git").exists():
            print(f"Not a git repository: {node_name}")
            return False

        print(f"Updating custom node {node_name}...")

        # Git pull
        success, stdout, stderr = run_command(
            ['git', 'pull'],
            cwd=node_path,
            timeout=60
        )

        if not success:
            print(f"Error updating custom node: {stderr}")
            return False

        print(f"✓ Updated custom node: {node_name}")
        print(stdout)

        # Check if requirements changed
        requirements_file = node_path / "requirements.txt"
        if requirements_file.exists():
            print(f"  Note: Check if requirements.txt changed")

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
            print(f"Custom node not found: {node_name}")
            return False

        try:
            shutil.rmtree(node_path)
            print(f"✓ Removed custom node: {node_name} from {version_tag}")
            return True
        except Exception as e:
            print(f"Error removing custom node: {e}")
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
        # Extract repo name from URL
        repo_name = git_url.rstrip('/').split('/')[-1]
        if repo_name.endswith('.git'):
            repo_name = repo_name[:-4]

        cache_path = self.shared_custom_nodes_cache_dir / f"{repo_name}.git"

        if cache_path.exists():
            # Update existing cache
            print(f"Updating cached repo: {repo_name}")
            success, stdout, stderr = run_command(
                ['git', 'fetch', '--all'],
                cwd=cache_path,
                timeout=60
            )

            if not success:
                print(f"Warning: Failed to update cache: {stderr}")

            return cache_path
        else:
            # Clone as bare repo
            print(f"Caching custom node repo: {repo_name}")
            ensure_directory(self.shared_custom_nodes_cache_dir)

            success, stdout, stderr = run_command(
                ['git', 'clone', '--bare', git_url, str(cache_path)],
                timeout=300
            )

            if not success:
                print(f"Error caching repo: {stderr}")
                return None

            print(f"✓ Cached repo: {repo_name}")
            return cache_path
