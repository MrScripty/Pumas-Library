"""Link registry for tracking symlinks and hardlinks.

Provides tracking and health monitoring for all links created
between the model library and application directories.
"""

from __future__ import annotations

import os
import sqlite3
from dataclasses import dataclass
from enum import Enum
from pathlib import Path
from typing import Any

from backend.logging_config import get_logger
from backend.models import get_iso_timestamp

logger = get_logger(__name__)


class LinkType(Enum):
    """Type of link stored in the registry."""

    SYMLINK = "symlink"
    HARDLINK = "hardlink"
    COPY = "copy"


class HealthStatus(Enum):
    """Overall health status for the link registry."""

    HEALTHY = "healthy"
    WARNINGS = "warnings"
    ERRORS = "errors"


@dataclass
class LinkInfo:
    """Information about a registered link.

    Attributes:
        link_id: Unique identifier for the link
        model_id: ID of the model this link points to
        source_path: Path to the source file in the library
        target_path: Path to the link in the application directory
        link_type: Type of link (symlink, hardlink, copy)
        app_id: Application identifier (e.g., 'comfyui')
        app_version: Application version (e.g., '0.6.0')
        is_external: Whether the link crosses filesystem boundaries
        created_at: ISO timestamp of when the link was created
    """

    link_id: int
    model_id: str
    source_path: str
    target_path: str
    link_type: LinkType
    app_id: str
    app_version: str
    is_external: bool
    created_at: str


@dataclass
class BrokenLinkInfo:
    """Information about a broken link.

    Attributes:
        link_id: Unique identifier for the link
        target_path: Path to the broken link
        expected_source: Expected source path (may no longer exist)
        model_id: ID of the model this link was supposed to point to
        reason: Human-readable reason why the link is broken
    """

    link_id: int
    target_path: str
    expected_source: str
    model_id: str
    reason: str


@dataclass
class HealthCheckResult:
    """Result of a health check on the link registry.

    Attributes:
        status: Overall health status
        total_links: Total number of registered links
        healthy_links: Number of valid, working links
        broken_links: List of broken link information
        orphaned_links: Links pointing to targets that don't exist in registry
        warnings: List of warning messages
        errors: List of error messages
    """

    status: HealthStatus
    total_links: int
    healthy_links: int
    broken_links: list[BrokenLinkInfo]
    orphaned_links: list[str]
    warnings: list[str]
    errors: list[str]


class LinkRegistry:
    """SQLite-backed registry for tracking model library links.

    Tracks all symlinks and hardlinks created between the model library
    and application directories (e.g., ComfyUI). Supports cascade delete,
    broken link detection, and health monitoring.

    Args:
        db_path: Path to the registry database file
    """

    def __init__(self, db_path: Path) -> None:
        self.db_path = Path(db_path)
        self._ensure_parent()
        self._ensure_schema()

    def _ensure_parent(self) -> None:
        """Ensure the parent directory exists."""
        self.db_path.parent.mkdir(parents=True, exist_ok=True)

    def _connect(self) -> sqlite3.Connection:
        """Create a database connection with WAL mode enabled."""
        conn = sqlite3.connect(self.db_path)
        conn.row_factory = sqlite3.Row
        # Enable WAL mode for better concurrent access
        conn.execute("PRAGMA journal_mode=WAL")
        return conn

    def _ensure_schema(self) -> None:
        """Create the database schema if it doesn't exist."""
        with self._connect() as conn:
            # Links table - tracks all created links
            conn.execute(
                """
                CREATE TABLE IF NOT EXISTS links (
                    link_id INTEGER PRIMARY KEY AUTOINCREMENT,
                    model_id TEXT NOT NULL,
                    source_path TEXT NOT NULL,
                    target_path TEXT NOT NULL UNIQUE,
                    link_type TEXT NOT NULL,
                    app_id TEXT NOT NULL,
                    app_version TEXT NOT NULL,
                    is_external INTEGER NOT NULL DEFAULT 0,
                    created_at TEXT NOT NULL
                )
                """
            )

            # Settings table - for storing app/library path tracking
            conn.execute(
                """
                CREATE TABLE IF NOT EXISTS settings (
                    key TEXT PRIMARY KEY,
                    value TEXT NOT NULL
                )
                """
            )

            # Indexes for common queries
            conn.execute("CREATE INDEX IF NOT EXISTS idx_links_model_id ON links(model_id)")
            conn.execute("CREATE INDEX IF NOT EXISTS idx_links_app ON links(app_id, app_version)")
            conn.execute("CREATE INDEX IF NOT EXISTS idx_links_is_external ON links(is_external)")

            conn.commit()

    def register_link(
        self,
        model_id: str,
        source_path: Path,
        target_path: Path,
        link_type: LinkType,
        app_id: str,
        app_version: str,
        is_external: bool = False,
    ) -> int:
        """Register a newly created link in the registry.

        Args:
            model_id: ID of the model being linked
            source_path: Path to the source file in the library
            target_path: Path to the link location
            link_type: Type of link (symlink, hardlink, etc.)
            app_id: Application identifier
            app_version: Application version
            is_external: Whether link crosses filesystem boundaries

        Returns:
            The link_id of the registered link

        Raises:
            sqlite3.IntegrityError: If target_path already exists in registry
        """
        created_at = get_iso_timestamp()

        with self._connect() as conn:
            cursor = conn.execute(
                """
                INSERT INTO links (
                    model_id, source_path, target_path, link_type,
                    app_id, app_version, is_external, created_at
                )
                VALUES (?, ?, ?, ?, ?, ?, ?, ?)
                """,
                (
                    model_id,
                    str(source_path),
                    str(target_path),
                    link_type.value,
                    app_id,
                    app_version,
                    1 if is_external else 0,
                    created_at,
                ),
            )
            conn.commit()
            link_id = cursor.lastrowid or 0

        logger.debug(
            "Registered link %d: %s -> %s (model=%s)",
            link_id,
            target_path,
            source_path,
            model_id,
        )
        return link_id

    def unregister_link(self, link_id: int) -> bool:
        """Remove a link from the registry.

        Args:
            link_id: ID of the link to remove

        Returns:
            True if the link was removed, False if not found
        """
        with self._connect() as conn:
            cursor = conn.execute("DELETE FROM links WHERE link_id = ?", (link_id,))
            conn.commit()
            return cursor.rowcount > 0

    def unregister_by_target(self, target_path: Path) -> bool:
        """Remove a link from the registry by target path.

        Args:
            target_path: Path to the link

        Returns:
            True if the link was removed, False if not found
        """
        with self._connect() as conn:
            cursor = conn.execute("DELETE FROM links WHERE target_path = ?", (str(target_path),))
            conn.commit()
            return cursor.rowcount > 0

    def get_links_for_model(self, model_id: str) -> list[LinkInfo]:
        """Get all links associated with a model.

        Args:
            model_id: ID of the model

        Returns:
            List of LinkInfo objects for all links to this model
        """
        with self._connect() as conn:
            rows = conn.execute("SELECT * FROM links WHERE model_id = ?", (model_id,)).fetchall()

        return [self._row_to_link_info(row) for row in rows]

    def get_links_for_app(self, app_id: str, app_version: str) -> list[LinkInfo]:
        """Get all links for a specific application version.

        Args:
            app_id: Application identifier
            app_version: Application version

        Returns:
            List of LinkInfo objects for all links to this app version
        """
        with self._connect() as conn:
            rows = conn.execute(
                "SELECT * FROM links WHERE app_id = ? AND app_version = ?",
                (app_id, app_version),
            ).fetchall()

        return [self._row_to_link_info(row) for row in rows]

    def get_link_by_target(self, target_path: Path) -> LinkInfo | None:
        """Get link information by target path.

        Args:
            target_path: Path to the link

        Returns:
            LinkInfo if found, None otherwise
        """
        with self._connect() as conn:
            row = conn.execute(
                "SELECT * FROM links WHERE target_path = ?", (str(target_path),)
            ).fetchone()

        if row is None:
            return None
        return self._row_to_link_info(row)

    def delete_links_for_model(self, model_id: str) -> int:
        """Delete all links associated with a model (cascade delete).

        This should be called before deleting a model from the library.
        It removes both the registry entries and the actual symlinks.

        Args:
            model_id: ID of the model

        Returns:
            Number of links removed
        """
        links = self.get_links_for_model(model_id)
        removed = 0

        for link in links:
            target = Path(link.target_path)
            try:
                if target.is_symlink():
                    target.unlink()
                    logger.debug("Removed symlink: %s", target)
                elif target.exists():
                    # Hardlink or copy - just remove the file
                    target.unlink()
                    logger.debug("Removed link/copy: %s", target)
            except OSError as e:
                logger.warning("Failed to remove link %s: %s", target, e)
                continue

            if self.unregister_link(link.link_id):
                removed += 1

        if removed > 0:
            logger.info("Cascade deleted %d links for model %s", removed, model_id)

        return removed

    def find_broken_links(self) -> list[BrokenLinkInfo]:
        """Find all links where the source file no longer exists.

        Returns:
            List of broken link information
        """
        broken: list[BrokenLinkInfo] = []

        with self._connect() as conn:
            rows = conn.execute(
                "SELECT link_id, model_id, source_path, target_path FROM links"
            ).fetchall()

        for row in rows:
            source = Path(row["source_path"])
            target = Path(row["target_path"])

            # Check if target exists (link itself)
            if not target.exists() and not target.is_symlink():
                broken.append(
                    BrokenLinkInfo(
                        link_id=row["link_id"],
                        target_path=row["target_path"],
                        expected_source=row["source_path"],
                        model_id=row["model_id"],
                        reason="Link target no longer exists",
                    )
                )
                continue

            # Check if it's a broken symlink
            if target.is_symlink():
                try:
                    target.resolve(strict=True)
                except OSError:  # noqa: no-except-logging
                    broken.append(
                        BrokenLinkInfo(
                            link_id=row["link_id"],
                            target_path=row["target_path"],
                            expected_source=row["source_path"],
                            model_id=row["model_id"],
                            reason="Broken symlink - source file missing",
                        )
                    )
                    continue

            # Check if source exists (for registry consistency)
            if not source.exists():
                broken.append(
                    BrokenLinkInfo(
                        link_id=row["link_id"],
                        target_path=row["target_path"],
                        expected_source=row["source_path"],
                        model_id=row["model_id"],
                        reason="Source file no longer exists in library",
                    )
                )

        return broken

    def find_orphaned_links(self, app_models_root: Path) -> list[str]:
        """Find symlinks in app directory not tracked in registry.

        Scans the application models directory for symlinks that
        aren't registered in this registry (orphaned links).

        Args:
            app_models_root: Root path of the application's models directory

        Returns:
            List of paths to orphaned symlinks
        """
        orphaned: list[str] = []

        if not app_models_root.exists():
            return orphaned

        # Get all registered target paths
        with self._connect() as conn:
            rows = conn.execute("SELECT target_path FROM links").fetchall()
        registered_targets = {row["target_path"] for row in rows}

        # Scan app directory for symlinks
        for item in app_models_root.rglob("*"):
            if item.is_symlink():
                if str(item) not in registered_targets:
                    orphaned.append(str(item))

        return orphaned

    def clean_broken_links(self) -> int:
        """Remove all broken links from the registry and filesystem.

        Returns:
            Number of broken links cleaned up
        """
        broken = self.find_broken_links()
        cleaned = 0

        for link in broken:
            target = Path(link.target_path)

            # Remove the actual symlink if it exists
            if target.is_symlink():
                try:
                    target.unlink()
                    logger.debug("Removed broken symlink: %s", target)
                except OSError as e:
                    logger.warning("Failed to remove broken symlink %s: %s", target, e)
                    continue

            # Remove from registry
            if self.unregister_link(link.link_id):
                cleaned += 1

        if cleaned > 0:
            logger.info("Cleaned up %d broken links", cleaned)

        return cleaned

    def remove_orphaned_links(self, app_models_root: Path) -> int:
        """Remove orphaned symlinks from the application directory.

        Args:
            app_models_root: Root path of the application's models directory

        Returns:
            Number of orphaned links removed
        """
        orphaned = self.find_orphaned_links(app_models_root)
        removed = 0

        for path_str in orphaned:
            path = Path(path_str)
            if path.is_symlink():
                try:
                    path.unlink()
                    removed += 1
                    logger.debug("Removed orphaned symlink: %s", path)
                except OSError as e:
                    logger.warning("Failed to remove orphaned symlink %s: %s", path, e)

        if removed > 0:
            logger.info("Removed %d orphaned symlinks", removed)

        return removed

    def perform_health_check(self, app_models_root: Path | None = None) -> HealthCheckResult:
        """Perform a comprehensive health check on the link registry.

        Args:
            app_models_root: Optional path to check for orphaned links

        Returns:
            HealthCheckResult with detailed status information
        """
        warnings: list[str] = []
        errors: list[str] = []

        # Count total links
        with self._connect() as conn:
            row = conn.execute("SELECT COUNT(*) as count FROM links").fetchone()
            total_links = row["count"] if row else 0

        # Find broken links
        broken_links = self.find_broken_links()
        if broken_links:
            errors.append(f"Found {len(broken_links)} broken links")

        # Find orphaned links (if path provided)
        orphaned_links: list[str] = []
        if app_models_root:
            orphaned_links = self.find_orphaned_links(app_models_root)
            if orphaned_links:
                warnings.append(f"Found {len(orphaned_links)} orphaned symlinks")

        # Check for external links
        with self._connect() as conn:
            row = conn.execute(
                "SELECT COUNT(*) as count FROM links WHERE is_external = 1"
            ).fetchone()
            external_count = row["count"] if row else 0

        if external_count > 0:
            warnings.append(
                f"{external_count} links cross filesystem boundaries "
                "(may break if drives are unmounted)"
            )

        # Calculate healthy links
        healthy_links = total_links - len(broken_links)

        # Determine overall status
        if errors:
            status = HealthStatus.ERRORS
        elif warnings:
            status = HealthStatus.WARNINGS
        else:
            status = HealthStatus.HEALTHY

        return HealthCheckResult(
            status=status,
            total_links=total_links,
            healthy_links=healthy_links,
            broken_links=broken_links,
            orphaned_links=orphaned_links,
            warnings=warnings,
            errors=errors,
        )

    def bulk_update_external_paths(self, old_prefix: str, new_prefix: str) -> int:
        """Update paths when an external drive mount point changes.

        Args:
            old_prefix: Old mount point prefix (e.g., '/media/user/OldDrive')
            new_prefix: New mount point prefix (e.g., '/media/user/NewDrive')

        Returns:
            Number of paths updated
        """
        updated = 0

        with self._connect() as conn:
            # Find affected links
            rows = conn.execute(
                "SELECT link_id, source_path, target_path FROM links "
                "WHERE source_path LIKE ? OR target_path LIKE ?",
                (f"{old_prefix}%", f"{old_prefix}%"),
            ).fetchall()

            for row in rows:
                new_source = row["source_path"]
                new_target = row["target_path"]

                if row["source_path"].startswith(old_prefix):
                    new_source = new_prefix + row["source_path"][len(old_prefix) :]

                if row["target_path"].startswith(old_prefix):
                    new_target = new_prefix + row["target_path"][len(old_prefix) :]

                if new_source != row["source_path"] or new_target != row["target_path"]:
                    conn.execute(
                        "UPDATE links SET source_path = ?, target_path = ? " "WHERE link_id = ?",
                        (new_source, new_target, row["link_id"]),
                    )
                    updated += 1

            conn.commit()

        if updated > 0:
            logger.info("Updated %d paths from %s to %s", updated, old_prefix, new_prefix)

        return updated

    def get_setting(self, key: str) -> str | None:
        """Get a setting value from the registry.

        Args:
            key: Setting key

        Returns:
            Setting value or None if not found
        """
        with self._connect() as conn:
            row = conn.execute("SELECT value FROM settings WHERE key = ?", (key,)).fetchone()
        return row["value"] if row else None

    def set_setting(self, key: str, value: str) -> None:
        """Set a setting value in the registry.

        Args:
            key: Setting key
            value: Setting value
        """
        with self._connect() as conn:
            conn.execute(
                "INSERT INTO settings (key, value) VALUES (?, ?) "
                "ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                (key, value),
            )
            conn.commit()

    def get_link_count(self) -> int:
        """Get the total number of registered links.

        Returns:
            Total link count
        """
        with self._connect() as conn:
            row = conn.execute("SELECT COUNT(*) as count FROM links").fetchone()
            return row["count"] if row else 0

    def clear(self) -> None:
        """Remove all links from the registry (for testing)."""
        with self._connect() as conn:
            conn.execute("DELETE FROM links")
            conn.commit()

    def _row_to_link_info(self, row: sqlite3.Row) -> LinkInfo:
        """Convert a database row to a LinkInfo object."""
        return LinkInfo(
            link_id=row["link_id"],
            model_id=row["model_id"],
            source_path=row["source_path"],
            target_path=row["target_path"],
            link_type=LinkType(row["link_type"]),
            app_id=row["app_id"],
            app_version=row["app_version"],
            is_external=bool(row["is_external"]),
            created_at=row["created_at"],
        )

    def to_dict(self, link_info: LinkInfo) -> dict[str, Any]:
        """Convert LinkInfo to a JSON-serializable dict.

        Args:
            link_info: LinkInfo object to convert

        Returns:
            Dictionary representation
        """
        return {
            "link_id": link_info.link_id,
            "model_id": link_info.model_id,
            "source_path": link_info.source_path,
            "target_path": link_info.target_path,
            "link_type": link_info.link_type.value,
            "app_id": link_info.app_id,
            "app_version": link_info.app_version,
            "is_external": link_info.is_external,
            "created_at": link_info.created_at,
        }
