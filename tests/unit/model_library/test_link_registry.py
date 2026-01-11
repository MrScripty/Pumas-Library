"""Tests for link registry module."""

from __future__ import annotations

import sqlite3
from pathlib import Path

import pytest

from backend.model_library.link_registry import (
    BrokenLinkInfo,
    HealthStatus,
    LinkInfo,
    LinkRegistry,
    LinkType,
)


@pytest.fixture
def registry(tmp_path: Path) -> LinkRegistry:
    """Create a fresh link registry for testing."""
    db_path = tmp_path / "registry.db"
    return LinkRegistry(db_path)


@pytest.fixture
def sample_source(tmp_path: Path) -> Path:
    """Create a sample source file."""
    source = tmp_path / "library" / "models" / "test-model" / "model.safetensors"
    source.parent.mkdir(parents=True, exist_ok=True)
    source.write_text("model content")
    return source


@pytest.fixture
def sample_target(tmp_path: Path) -> Path:
    """Create a sample target directory."""
    target_dir = tmp_path / "app" / "models" / "checkpoints"
    target_dir.mkdir(parents=True, exist_ok=True)
    return target_dir / "model.safetensors"


@pytest.mark.unit
class TestLinkType:
    """Tests for LinkType enum."""

    def test_link_type_values(self):
        """Test that link types have expected values."""
        assert LinkType.SYMLINK.value == "symlink"
        assert LinkType.HARDLINK.value == "hardlink"
        assert LinkType.COPY.value == "copy"

    def test_link_type_from_string(self):
        """Test creating link type from string."""
        assert LinkType("symlink") == LinkType.SYMLINK
        assert LinkType("hardlink") == LinkType.HARDLINK
        assert LinkType("copy") == LinkType.COPY


@pytest.mark.unit
class TestHealthStatus:
    """Tests for HealthStatus enum."""

    def test_health_status_values(self):
        """Test that health statuses have expected values."""
        assert HealthStatus.HEALTHY.value == "healthy"
        assert HealthStatus.WARNINGS.value == "warnings"
        assert HealthStatus.ERRORS.value == "errors"


@pytest.mark.unit
class TestLinkInfo:
    """Tests for LinkInfo dataclass."""

    def test_create_link_info(self):
        """Test creating a LinkInfo object."""
        info = LinkInfo(
            link_id=1,
            model_id="test-model",
            source_path="/library/model.safetensors",
            target_path="/app/model.safetensors",
            link_type=LinkType.SYMLINK,
            app_id="comfyui",
            app_version="0.6.0",
            is_external=False,
            created_at="2026-01-11T00:00:00Z",
        )
        assert info.link_id == 1
        assert info.model_id == "test-model"
        assert info.link_type == LinkType.SYMLINK
        assert not info.is_external


@pytest.mark.unit
class TestBrokenLinkInfo:
    """Tests for BrokenLinkInfo dataclass."""

    def test_create_broken_link_info(self):
        """Test creating a BrokenLinkInfo object."""
        info = BrokenLinkInfo(
            link_id=1,
            target_path="/app/model.safetensors",
            expected_source="/library/model.safetensors",
            model_id="test-model",
            reason="Source file missing",
        )
        assert info.link_id == 1
        assert info.reason == "Source file missing"


@pytest.mark.unit
class TestLinkRegistryInit:
    """Tests for LinkRegistry initialization."""

    def test_creates_database(self, tmp_path: Path):
        """Test that registry creates database file."""
        db_path = tmp_path / "test.db"
        LinkRegistry(db_path)
        assert db_path.exists()

    def test_creates_parent_directory(self, tmp_path: Path):
        """Test that registry creates parent directories."""
        db_path = tmp_path / "nested" / "path" / "registry.db"
        LinkRegistry(db_path)
        assert db_path.exists()

    def test_creates_schema(self, tmp_path: Path):
        """Test that registry creates expected tables."""
        db_path = tmp_path / "test.db"
        LinkRegistry(db_path)

        conn = sqlite3.connect(db_path)
        cursor = conn.execute("SELECT name FROM sqlite_master WHERE type='table'")
        tables = {row[0] for row in cursor.fetchall()}
        conn.close()

        assert "links" in tables
        assert "settings" in tables

    def test_wal_mode_enabled(self, tmp_path: Path):
        """Test that WAL mode is enabled for the database."""
        db_path = tmp_path / "test.db"
        registry = LinkRegistry(db_path)

        conn = registry._connect()
        cursor = conn.execute("PRAGMA journal_mode")
        mode = cursor.fetchone()[0]
        conn.close()

        assert mode == "wal"


@pytest.mark.unit
class TestRegisterLink:
    """Tests for link registration."""

    def test_register_symlink(
        self, registry: LinkRegistry, sample_source: Path, sample_target: Path
    ):
        """Test registering a symlink."""
        link_id = registry.register_link(
            model_id="test-model",
            source_path=sample_source,
            target_path=sample_target,
            link_type=LinkType.SYMLINK,
            app_id="comfyui",
            app_version="0.6.0",
        )

        assert link_id > 0

    def test_register_link_returns_unique_ids(
        self, registry: LinkRegistry, sample_source: Path, tmp_path: Path
    ):
        """Test that each registration gets a unique ID."""
        ids = []
        for i in range(3):
            target = tmp_path / "app" / f"link{i}.safetensors"
            target.parent.mkdir(parents=True, exist_ok=True)
            link_id = registry.register_link(
                model_id="test-model",
                source_path=sample_source,
                target_path=target,
                link_type=LinkType.SYMLINK,
                app_id="comfyui",
                app_version="0.6.0",
            )
            ids.append(link_id)

        assert len(set(ids)) == 3

    def test_register_hardlink(
        self, registry: LinkRegistry, sample_source: Path, sample_target: Path
    ):
        """Test registering a hardlink."""
        link_id = registry.register_link(
            model_id="test-model",
            source_path=sample_source,
            target_path=sample_target,
            link_type=LinkType.HARDLINK,
            app_id="comfyui",
            app_version="0.6.0",
        )

        link = registry.get_link_by_target(sample_target)
        assert link is not None
        assert link.link_type == LinkType.HARDLINK

    def test_register_external_link(
        self, registry: LinkRegistry, sample_source: Path, sample_target: Path
    ):
        """Test registering an external (cross-filesystem) link."""
        link_id = registry.register_link(
            model_id="test-model",
            source_path=sample_source,
            target_path=sample_target,
            link_type=LinkType.SYMLINK,
            app_id="comfyui",
            app_version="0.6.0",
            is_external=True,
        )

        link = registry.get_link_by_target(sample_target)
        assert link is not None
        assert link.is_external is True

    def test_register_duplicate_target_fails(
        self, registry: LinkRegistry, sample_source: Path, sample_target: Path
    ):
        """Test that registering duplicate target path fails."""
        registry.register_link(
            model_id="test-model",
            source_path=sample_source,
            target_path=sample_target,
            link_type=LinkType.SYMLINK,
            app_id="comfyui",
            app_version="0.6.0",
        )

        with pytest.raises(sqlite3.IntegrityError):
            registry.register_link(
                model_id="other-model",
                source_path=sample_source,
                target_path=sample_target,  # Same target
                link_type=LinkType.SYMLINK,
                app_id="comfyui",
                app_version="0.6.0",
            )


@pytest.mark.unit
class TestUnregisterLink:
    """Tests for link unregistration."""

    def test_unregister_by_id(
        self, registry: LinkRegistry, sample_source: Path, sample_target: Path
    ):
        """Test unregistering a link by ID."""
        link_id = registry.register_link(
            model_id="test-model",
            source_path=sample_source,
            target_path=sample_target,
            link_type=LinkType.SYMLINK,
            app_id="comfyui",
            app_version="0.6.0",
        )

        result = registry.unregister_link(link_id)
        assert result is True
        assert registry.get_link_by_target(sample_target) is None

    def test_unregister_nonexistent_returns_false(self, registry: LinkRegistry):
        """Test that unregistering nonexistent link returns False."""
        result = registry.unregister_link(99999)
        assert result is False

    def test_unregister_by_target(
        self, registry: LinkRegistry, sample_source: Path, sample_target: Path
    ):
        """Test unregistering a link by target path."""
        registry.register_link(
            model_id="test-model",
            source_path=sample_source,
            target_path=sample_target,
            link_type=LinkType.SYMLINK,
            app_id="comfyui",
            app_version="0.6.0",
        )

        result = registry.unregister_by_target(sample_target)
        assert result is True
        assert registry.get_link_by_target(sample_target) is None


@pytest.mark.unit
class TestGetLinks:
    """Tests for retrieving links."""

    def test_get_links_for_model(self, registry: LinkRegistry, sample_source: Path, tmp_path: Path):
        """Test getting all links for a model."""
        targets = []
        for i in range(3):
            target = tmp_path / "app" / f"link{i}.safetensors"
            target.parent.mkdir(parents=True, exist_ok=True)
            targets.append(target)
            registry.register_link(
                model_id="test-model",
                source_path=sample_source,
                target_path=target,
                link_type=LinkType.SYMLINK,
                app_id="comfyui",
                app_version="0.6.0",
            )

        links = registry.get_links_for_model("test-model")
        assert len(links) == 3

    def test_get_links_for_model_empty(self, registry: LinkRegistry):
        """Test getting links for nonexistent model."""
        links = registry.get_links_for_model("nonexistent")
        assert links == []

    def test_get_links_for_app(self, registry: LinkRegistry, sample_source: Path, tmp_path: Path):
        """Test getting all links for an app version."""
        for i in range(2):
            target = tmp_path / "app" / f"link{i}.safetensors"
            target.parent.mkdir(parents=True, exist_ok=True)
            registry.register_link(
                model_id=f"model-{i}",
                source_path=sample_source,
                target_path=target,
                link_type=LinkType.SYMLINK,
                app_id="comfyui",
                app_version="0.6.0",
            )

        links = registry.get_links_for_app("comfyui", "0.6.0")
        assert len(links) == 2

    def test_get_link_by_target(
        self, registry: LinkRegistry, sample_source: Path, sample_target: Path
    ):
        """Test getting a link by target path."""
        registry.register_link(
            model_id="test-model",
            source_path=sample_source,
            target_path=sample_target,
            link_type=LinkType.SYMLINK,
            app_id="comfyui",
            app_version="0.6.0",
        )

        link = registry.get_link_by_target(sample_target)
        assert link is not None
        assert link.model_id == "test-model"
        assert link.source_path == str(sample_source)

    def test_get_link_by_target_not_found(self, registry: LinkRegistry, tmp_path: Path):
        """Test getting nonexistent link returns None."""
        link = registry.get_link_by_target(tmp_path / "nonexistent")
        assert link is None


@pytest.mark.unit
class TestCascadeDelete:
    """Tests for cascade delete functionality."""

    def test_delete_links_for_model_removes_symlinks(
        self, registry: LinkRegistry, sample_source: Path, tmp_path: Path
    ):
        """Test that cascade delete removes actual symlinks."""
        target = tmp_path / "app" / "model.safetensors"
        target.parent.mkdir(parents=True, exist_ok=True)

        # Create actual symlink
        target.symlink_to(sample_source)

        # Register in registry
        registry.register_link(
            model_id="test-model",
            source_path=sample_source,
            target_path=target,
            link_type=LinkType.SYMLINK,
            app_id="comfyui",
            app_version="0.6.0",
        )

        # Cascade delete
        removed = registry.delete_links_for_model("test-model")

        assert removed == 1
        assert not target.exists()
        assert not target.is_symlink()
        assert registry.get_links_for_model("test-model") == []

    def test_delete_links_for_model_multiple_links(
        self, registry: LinkRegistry, sample_source: Path, tmp_path: Path
    ):
        """Test cascade delete with multiple links."""
        targets = []
        for i in range(3):
            target = tmp_path / "app" / f"link{i}.safetensors"
            target.parent.mkdir(parents=True, exist_ok=True)
            target.symlink_to(sample_source)
            targets.append(target)
            registry.register_link(
                model_id="test-model",
                source_path=sample_source,
                target_path=target,
                link_type=LinkType.SYMLINK,
                app_id="comfyui",
                app_version="0.6.0",
            )

        removed = registry.delete_links_for_model("test-model")

        assert removed == 3
        for target in targets:
            assert not target.exists()

    def test_delete_links_preserves_source(
        self, registry: LinkRegistry, sample_source: Path, tmp_path: Path
    ):
        """Test that cascade delete doesn't affect source files."""
        target = tmp_path / "app" / "model.safetensors"
        target.parent.mkdir(parents=True, exist_ok=True)
        target.symlink_to(sample_source)

        registry.register_link(
            model_id="test-model",
            source_path=sample_source,
            target_path=target,
            link_type=LinkType.SYMLINK,
            app_id="comfyui",
            app_version="0.6.0",
        )

        registry.delete_links_for_model("test-model")

        # Source should still exist
        assert sample_source.exists()


@pytest.mark.unit
class TestFindBrokenLinks:
    """Tests for broken link detection."""

    def test_finds_broken_symlink(
        self, registry: LinkRegistry, sample_source: Path, tmp_path: Path
    ):
        """Test detecting a broken symlink."""
        target = tmp_path / "app" / "model.safetensors"
        target.parent.mkdir(parents=True, exist_ok=True)
        target.symlink_to(sample_source)

        registry.register_link(
            model_id="test-model",
            source_path=sample_source,
            target_path=target,
            link_type=LinkType.SYMLINK,
            app_id="comfyui",
            app_version="0.6.0",
        )

        # Delete source to break the link
        sample_source.unlink()

        broken = registry.find_broken_links()

        assert len(broken) == 1
        assert broken[0].target_path == str(target)
        assert "broken" in broken[0].reason.lower() or "missing" in broken[0].reason.lower()

    def test_finds_missing_target(
        self, registry: LinkRegistry, sample_source: Path, tmp_path: Path
    ):
        """Test detecting when link target no longer exists."""
        target = tmp_path / "app" / "model.safetensors"
        target.parent.mkdir(parents=True, exist_ok=True)

        # Register but don't create actual link
        registry.register_link(
            model_id="test-model",
            source_path=sample_source,
            target_path=target,
            link_type=LinkType.SYMLINK,
            app_id="comfyui",
            app_version="0.6.0",
        )

        broken = registry.find_broken_links()

        assert len(broken) == 1
        assert broken[0].target_path == str(target)

    def test_healthy_link_not_reported(
        self, registry: LinkRegistry, sample_source: Path, tmp_path: Path
    ):
        """Test that healthy links are not reported as broken."""
        target = tmp_path / "app" / "model.safetensors"
        target.parent.mkdir(parents=True, exist_ok=True)
        target.symlink_to(sample_source)

        registry.register_link(
            model_id="test-model",
            source_path=sample_source,
            target_path=target,
            link_type=LinkType.SYMLINK,
            app_id="comfyui",
            app_version="0.6.0",
        )

        broken = registry.find_broken_links()
        assert len(broken) == 0


@pytest.mark.unit
class TestFindOrphanedLinks:
    """Tests for orphaned link detection."""

    def test_finds_orphaned_symlink(
        self, registry: LinkRegistry, sample_source: Path, tmp_path: Path
    ):
        """Test detecting symlinks not in registry."""
        app_root = tmp_path / "app" / "models"
        app_root.mkdir(parents=True, exist_ok=True)

        # Create symlink without registering
        orphan = app_root / "orphan.safetensors"
        orphan.symlink_to(sample_source)

        orphaned = registry.find_orphaned_links(app_root)

        assert len(orphaned) == 1
        assert str(orphan) in orphaned

    def test_registered_link_not_orphaned(
        self, registry: LinkRegistry, sample_source: Path, tmp_path: Path
    ):
        """Test that registered links are not reported as orphaned."""
        app_root = tmp_path / "app" / "models"
        target = app_root / "registered.safetensors"
        target.parent.mkdir(parents=True, exist_ok=True)
        target.symlink_to(sample_source)

        registry.register_link(
            model_id="test-model",
            source_path=sample_source,
            target_path=target,
            link_type=LinkType.SYMLINK,
            app_id="comfyui",
            app_version="0.6.0",
        )

        orphaned = registry.find_orphaned_links(app_root)
        assert len(orphaned) == 0


@pytest.mark.unit
class TestCleanBrokenLinks:
    """Tests for cleaning broken links."""

    def test_clean_removes_broken_symlinks(
        self, registry: LinkRegistry, sample_source: Path, tmp_path: Path
    ):
        """Test that cleaning removes broken symlinks from filesystem."""
        target = tmp_path / "app" / "model.safetensors"
        target.parent.mkdir(parents=True, exist_ok=True)
        target.symlink_to(sample_source)

        registry.register_link(
            model_id="test-model",
            source_path=sample_source,
            target_path=target,
            link_type=LinkType.SYMLINK,
            app_id="comfyui",
            app_version="0.6.0",
        )

        # Break the link
        sample_source.unlink()

        cleaned = registry.clean_broken_links()

        assert cleaned == 1
        assert not target.is_symlink()
        assert registry.get_link_count() == 0


@pytest.mark.unit
class TestRemoveOrphanedLinks:
    """Tests for removing orphaned links."""

    def test_removes_orphaned_symlinks(
        self, registry: LinkRegistry, sample_source: Path, tmp_path: Path
    ):
        """Test that orphaned symlinks are removed."""
        app_root = tmp_path / "app" / "models"
        app_root.mkdir(parents=True, exist_ok=True)

        orphan = app_root / "orphan.safetensors"
        orphan.symlink_to(sample_source)

        removed = registry.remove_orphaned_links(app_root)

        assert removed == 1
        assert not orphan.exists()


@pytest.mark.unit
class TestHealthCheck:
    """Tests for health check functionality."""

    def test_healthy_status_when_no_issues(
        self, registry: LinkRegistry, sample_source: Path, tmp_path: Path
    ):
        """Test healthy status when all links are valid."""
        target = tmp_path / "app" / "model.safetensors"
        target.parent.mkdir(parents=True, exist_ok=True)
        target.symlink_to(sample_source)

        registry.register_link(
            model_id="test-model",
            source_path=sample_source,
            target_path=target,
            link_type=LinkType.SYMLINK,
            app_id="comfyui",
            app_version="0.6.0",
        )

        result = registry.perform_health_check()

        assert result.status == HealthStatus.HEALTHY
        assert result.total_links == 1
        assert result.healthy_links == 1
        assert len(result.broken_links) == 0

    def test_errors_status_when_broken_links(
        self, registry: LinkRegistry, sample_source: Path, tmp_path: Path
    ):
        """Test errors status when broken links exist."""
        target = tmp_path / "app" / "model.safetensors"
        target.parent.mkdir(parents=True, exist_ok=True)
        target.symlink_to(sample_source)

        registry.register_link(
            model_id="test-model",
            source_path=sample_source,
            target_path=target,
            link_type=LinkType.SYMLINK,
            app_id="comfyui",
            app_version="0.6.0",
        )

        # Break the link
        sample_source.unlink()

        result = registry.perform_health_check()

        assert result.status == HealthStatus.ERRORS
        assert len(result.errors) > 0

    def test_warnings_status_when_orphaned_links(
        self, registry: LinkRegistry, sample_source: Path, tmp_path: Path
    ):
        """Test warnings status when orphaned links exist."""
        app_root = tmp_path / "app" / "models"
        app_root.mkdir(parents=True, exist_ok=True)

        orphan = app_root / "orphan.safetensors"
        orphan.symlink_to(sample_source)

        result = registry.perform_health_check(app_models_root=app_root)

        assert result.status == HealthStatus.WARNINGS
        assert len(result.warnings) > 0
        assert len(result.orphaned_links) == 1

    def test_warnings_for_external_links(
        self, registry: LinkRegistry, sample_source: Path, tmp_path: Path
    ):
        """Test that external links trigger warnings."""
        target = tmp_path / "app" / "model.safetensors"
        target.parent.mkdir(parents=True, exist_ok=True)
        target.symlink_to(sample_source)

        registry.register_link(
            model_id="test-model",
            source_path=sample_source,
            target_path=target,
            link_type=LinkType.SYMLINK,
            app_id="comfyui",
            app_version="0.6.0",
            is_external=True,
        )

        result = registry.perform_health_check()

        assert result.status == HealthStatus.WARNINGS
        assert any("external" in w.lower() or "cross" in w.lower() for w in result.warnings)


@pytest.mark.unit
class TestBulkUpdatePaths:
    """Tests for bulk path updates."""

    def test_updates_source_paths(self, registry: LinkRegistry, tmp_path: Path):
        """Test updating source paths with new mount point."""
        old_prefix = "/media/user/OldDrive"
        new_prefix = "/media/user/NewDrive"

        # Register with old paths
        registry.register_link(
            model_id="test-model",
            source_path=Path(f"{old_prefix}/models/model.safetensors"),
            target_path=tmp_path / "app" / "model.safetensors",
            link_type=LinkType.SYMLINK,
            app_id="comfyui",
            app_version="0.6.0",
        )

        updated = registry.bulk_update_external_paths(old_prefix, new_prefix)

        assert updated == 1

        link = registry.get_link_by_target(tmp_path / "app" / "model.safetensors")
        assert link is not None
        assert link.source_path.startswith(new_prefix)

    def test_no_update_when_prefix_not_matched(
        self, registry: LinkRegistry, sample_source: Path, sample_target: Path
    ):
        """Test that paths not matching prefix are not updated."""
        registry.register_link(
            model_id="test-model",
            source_path=sample_source,
            target_path=sample_target,
            link_type=LinkType.SYMLINK,
            app_id="comfyui",
            app_version="0.6.0",
        )

        updated = registry.bulk_update_external_paths("/nonexistent/prefix", "/new/prefix")

        assert updated == 0


@pytest.mark.unit
class TestSettings:
    """Tests for settings storage."""

    def test_set_and_get_setting(self, registry: LinkRegistry):
        """Test setting and getting a value."""
        registry.set_setting("test_key", "test_value")
        value = registry.get_setting("test_key")
        assert value == "test_value"

    def test_get_nonexistent_setting(self, registry: LinkRegistry):
        """Test getting nonexistent setting returns None."""
        value = registry.get_setting("nonexistent")
        assert value is None

    def test_update_existing_setting(self, registry: LinkRegistry):
        """Test updating an existing setting."""
        registry.set_setting("key", "value1")
        registry.set_setting("key", "value2")
        assert registry.get_setting("key") == "value2"


@pytest.mark.unit
class TestLinkCount:
    """Tests for link counting."""

    def test_get_link_count_empty(self, registry: LinkRegistry):
        """Test count with empty registry."""
        assert registry.get_link_count() == 0

    def test_get_link_count_with_links(
        self, registry: LinkRegistry, sample_source: Path, tmp_path: Path
    ):
        """Test count with registered links."""
        for i in range(5):
            target = tmp_path / "app" / f"link{i}.safetensors"
            target.parent.mkdir(parents=True, exist_ok=True)
            registry.register_link(
                model_id=f"model-{i}",
                source_path=sample_source,
                target_path=target,
                link_type=LinkType.SYMLINK,
                app_id="comfyui",
                app_version="0.6.0",
            )

        assert registry.get_link_count() == 5


@pytest.mark.unit
class TestClear:
    """Tests for clearing the registry."""

    def test_clear_removes_all_links(
        self, registry: LinkRegistry, sample_source: Path, tmp_path: Path
    ):
        """Test that clear removes all links from registry."""
        for i in range(3):
            target = tmp_path / "app" / f"link{i}.safetensors"
            target.parent.mkdir(parents=True, exist_ok=True)
            registry.register_link(
                model_id=f"model-{i}",
                source_path=sample_source,
                target_path=target,
                link_type=LinkType.SYMLINK,
                app_id="comfyui",
                app_version="0.6.0",
            )

        assert registry.get_link_count() == 3

        registry.clear()

        assert registry.get_link_count() == 0


@pytest.mark.unit
class TestToDict:
    """Tests for LinkInfo serialization."""

    def test_to_dict_conversion(
        self, registry: LinkRegistry, sample_source: Path, sample_target: Path
    ):
        """Test converting LinkInfo to dict."""
        registry.register_link(
            model_id="test-model",
            source_path=sample_source,
            target_path=sample_target,
            link_type=LinkType.SYMLINK,
            app_id="comfyui",
            app_version="0.6.0",
        )

        link = registry.get_link_by_target(sample_target)
        assert link is not None

        data = registry.to_dict(link)

        assert data["model_id"] == "test-model"
        assert data["link_type"] == "symlink"
        assert data["app_id"] == "comfyui"
        assert data["app_version"] == "0.6.0"
        assert isinstance(data["is_external"], bool)
