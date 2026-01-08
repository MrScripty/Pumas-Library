"""Installed version state helpers for VersionManager."""

from __future__ import annotations

import shutil
from pathlib import Path
from typing import Any, Dict, List, Optional

from backend.logging_config import get_logger
from backend.models import VersionInfo
from backend.validators import validate_version_tag
from backend.version_manager_components.protocols import MixinBase, StateContext

logger = get_logger(__name__)


class StateMixin(MixinBase, StateContext):
    """Mix-in for installed version state and metadata management."""

    def _write_active_version_file(self, tag: Optional[str]) -> bool:
        """Persist active version tag to file or clear it when None."""
        try:
            if tag:
                self.active_version_file.write_text(tag)
            else:
                if self.active_version_file.exists():
                    self.active_version_file.unlink()
            return True
        except OSError as exc:
            logger.error(f"Error writing active version file: {exc}", exc_info=True)
            return False

    def _set_active_version_state(self, tag: Optional[str], update_last_selected: bool) -> bool:
        """
        Update in-memory and on-disk active version state.

        Args:
            tag: Version tag to mark active, or None
            update_last_selected: When True, persist as lastSelectedVersion (user choice)
        """
        self._active_version = tag
        success = self._write_active_version_file(tag)

        if update_last_selected:
            versions_metadata = self.metadata_manager.load_versions()
            versions_metadata["lastSelectedVersion"] = tag
            success = self.metadata_manager.save_versions(versions_metadata) and success

        return success

    def _initialize_active_version(self) -> Optional[str]:
        """
        Set the startup active version using priority:
        1) defaultVersion
        2) lastSelectedVersion
        3) newest installed
        """
        installed_versions = self.get_installed_versions()
        if not installed_versions:
            self._set_active_version_state(None, update_last_selected=False)
            return None

        versions_metadata = self.metadata_manager.load_versions()
        candidates = [
            versions_metadata.get("defaultVersion"),
            versions_metadata.get("lastSelectedVersion"),
        ]

        for candidate in candidates:
            if candidate and candidate in installed_versions:
                self._set_active_version_state(candidate, update_last_selected=False)
                return candidate

        newest = sorted(installed_versions, reverse=True)[0]
        self._set_active_version_state(newest, update_last_selected=False)
        return newest

    def get_installed_versions(self) -> List[str]:
        """
        Get list of installed version tags (validated against actual directories)

        Returns:
            List of version tags that are both in metadata and have valid directories
        """
        versions_metadata = self.metadata_manager.load_versions()
        metadata_versions = set(versions_metadata.get("installed", {}).keys())

        # Verify each version actually exists on disk
        validated_versions = []
        needs_cleanup = False

        for tag in metadata_versions:
            version_path = self.versions_dir / tag
            if self._is_version_complete(version_path):
                validated_versions.append(tag)
            else:
                # Version is in metadata but incomplete/missing on disk
                logger.warning(f"Version {tag} is incomplete or missing, removing from metadata")
                needs_cleanup = True

        # Clean up metadata if we found incomplete versions
        # NOTE: This ONLY modifies the 'installed' dict in versions.json
        # It does NOT touch the GitHub releases cache or any other cache files
        if needs_cleanup:
            for tag in metadata_versions:
                if tag not in validated_versions:
                    del versions_metadata["installed"][tag]
            self.metadata_manager.save_versions(versions_metadata)
            logger.info(
                "✓ Cleaned up metadata - removed "
                f"{len(metadata_versions) - len(validated_versions)} incomplete version(s)"
            )

        return validated_versions

    def validate_installations(self) -> Dict[str, Any]:
        """
        Validate all installations and return cleanup report

        This is meant to be called at startup to detect and clean up
        any incomplete installations, and report back to the frontend
        so it can refresh the UI if needed.

        Checks two scenarios:
        1. Metadata says installed, but directory is incomplete/missing
        2. Directory exists, but no metadata (cancelled/interrupted install)

        Returns:
            Dict with:
                - had_invalid: bool - whether any invalid installations were found
                - removed: List[str] - tags of removed versions
                - valid: List[str] - tags of valid installed versions
        """
        versions_metadata = self.metadata_manager.load_versions()
        metadata_versions = set(versions_metadata.get("installed", {}).keys())

        validated_versions = []
        removed_versions = []

        # Check 1: Validate versions in metadata
        for tag in metadata_versions:
            version_path = self.versions_dir / tag
            if self._is_version_complete(version_path):
                validated_versions.append(tag)
            else:
                removed_versions.append(tag)
                logger.warning(f"Version {tag} in metadata but directory incomplete/missing")

        # Check 2: Look for orphaned directories (no metadata = incomplete install)
        if self.versions_dir.exists():
            for version_dir in self.versions_dir.iterdir():
                if version_dir.is_dir():
                    tag = version_dir.name
                    # If directory exists but NOT in metadata, it's an incomplete install
                    if tag not in metadata_versions:
                        removed_versions.append(tag)
                        logger.warning(
                            f"Found incomplete installation directory: {tag} (not in metadata)"
                        )
                        # Remove the orphaned directory
                        try:
                            shutil.rmtree(version_dir)
                            logger.info(f"✓ Removed incomplete installation directory: {tag}")
                        except OSError as e:
                            logger.error(f"Error removing {tag}: {e}", exc_info=True)

        # Clean up metadata if we found incomplete versions in metadata
        if any(tag in metadata_versions for tag in removed_versions):
            for tag in removed_versions:
                if tag in versions_metadata["installed"]:
                    del versions_metadata["installed"][tag]
            self.metadata_manager.save_versions(versions_metadata)
            logger.info(
                f"✓ Cleaned up {len(removed_versions)} incomplete installation(s): "
                f"{', '.join(removed_versions)}"
            )

        return {
            "had_invalid": len(removed_versions) > 0,
            "removed": removed_versions,
            "valid": validated_versions,
        }

    def _is_version_complete(self, version_path: Path) -> bool:
        """
        Check if a version installation is complete

        Args:
            version_path: Path to version directory

        Returns:
            True if version appears complete
        """
        if not version_path.exists():
            return False

        # Check for essential files/directories
        required_paths = [
            version_path / "main.py",  # Core ComfyUI file
            version_path / "venv",  # Virtual environment
            version_path / "venv" / "bin" / "python",  # Python in venv
        ]

        for path in required_paths:
            if not path.exists():
                return False

        return True

    def get_version_info(self, tag: str) -> Optional[VersionInfo]:
        """
        Get info about an installed version

        Args:
            tag: Version tag

        Returns:
            VersionInfo or None if not installed
        """
        if not validate_version_tag(tag):
            logger.warning(f"Invalid version tag for info lookup: {tag!r}")
            return None
        versions_metadata = self.metadata_manager.load_versions()
        return versions_metadata.get("installed", {}).get(tag)

    def get_version_path(self, tag: str) -> Optional[Path]:
        """
        Get filesystem path for an installed version.

        Args:
            tag: Version tag

        Returns:
            Path to version directory or None if missing/incomplete
        """
        if not validate_version_tag(tag):
            logger.warning(f"Invalid version tag for path lookup: {tag!r}")
            return None

        version_path = self.versions_dir / tag
        if not version_path.exists():
            return None

        if not self._is_version_complete(version_path):
            return None

        return version_path

    def get_active_version(self) -> Optional[str]:
        """
        Get currently active version tag for this session.

        If the session has no active version (or it points to a missing install),
        re-evaluate using startup priority: defaultVersion → lastSelectedVersion
        → newest installed.

        Returns:
            Active version tag or None
        """
        installed_versions = self.get_installed_versions()

        # If no versions installed, return None
        if not installed_versions:
            self._active_version = None
            return None

        # Honor current session selection if still valid
        if self._active_version in installed_versions:
            return self._active_version

        # Re-evaluate using startup priority when session state is missing/stale
        return self._initialize_active_version()

    def get_active_version_path(self) -> Optional[Path]:
        """
        Get filesystem path for the active version.

        Returns:
            Path or None if no active version or incomplete installation
        """
        active_tag = self.get_active_version()
        if not active_tag:
            return None

        return self.get_version_path(active_tag)

    def set_active_version(self, tag: str) -> bool:
        """
        Set a version as active

        Args:
            tag: Version tag to activate

        Returns:
            True if successful
        """
        if not validate_version_tag(tag):
            logger.warning(f"Invalid version tag for activation: {tag!r}")
            return False

        # Verify version is installed
        if tag not in self.get_installed_versions():
            logger.warning(f"Version {tag} is not installed")
            return False

        # Update active version state (persist as user choice)
        if not self._set_active_version_state(tag, update_last_selected=True):
            return False

        logger.info(f"✓ Activated version: {tag}")
        return True

    def get_default_version(self) -> Optional[str]:
        """
        Get the default version set in metadata.
        """
        versions_metadata = self.metadata_manager.load_versions()
        return versions_metadata.get("defaultVersion")

    def set_default_version(self, tag: Optional[str]) -> bool:
        """
        Set a version as default (or clear if tag is None).
        """
        if tag is not None and not validate_version_tag(tag):
            logger.warning(f"Invalid version tag for default selection: {tag!r}")
            return False
        versions_metadata = self.metadata_manager.load_versions()
        installed = versions_metadata.get("installed", {})

        if tag is not None and tag not in installed:
            logger.warning(f"Cannot set default to {tag}: not installed")
            return False

        versions_metadata["defaultVersion"] = tag
        self.metadata_manager.save_versions(versions_metadata)
        logger.info(f"✓ Default version set to: {tag}")
        return True

    def get_version_status(self) -> Dict[str, Any]:
        """
        Get comprehensive status of all versions

        Returns:
            Dict with version status information
        """
        installed = self.get_installed_versions()
        active = self.get_active_version()

        versions_status: Dict[str, Any] = {}
        status: Dict[str, Any] = {
            "installedCount": len(installed),
            "activeVersion": active,
            "defaultVersion": self.get_default_version(),
            "versions": versions_status,
        }

        for tag in installed:
            version_info = self.get_version_info(tag)
            dep_status = self.check_dependencies(tag)

            versions_status[tag] = {
                "info": version_info,
                "dependencies": dep_status,
                "isActive": tag == active,
            }

        return status
