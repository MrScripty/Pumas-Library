"""Tests for FTS5 virtual table setup and migration."""

from __future__ import annotations

import sqlite3
import tempfile
from pathlib import Path
from typing import Any

import pytest

from backend.model_library.search.fts5 import (
    FTS5Config,
    FTS5Manager,
    create_fts5_table,
    create_fts5_triggers,
    drop_fts5_triggers,
    fts5_table_exists,
    migrate_to_fts5,
    populate_fts5_from_models,
)


@pytest.fixture
def temp_db() -> sqlite3.Connection:
    """Create a temporary in-memory database with models table."""
    conn = sqlite3.connect(":memory:")
    conn.row_factory = sqlite3.Row
    conn.execute(
        """
        CREATE TABLE models (
            id TEXT PRIMARY KEY,
            path TEXT NOT NULL,
            cleaned_name TEXT NOT NULL,
            official_name TEXT NOT NULL,
            model_type TEXT NOT NULL,
            tags_json TEXT NOT NULL,
            hashes_json TEXT NOT NULL,
            metadata_json TEXT NOT NULL,
            updated_at TEXT NOT NULL
        )
    """
    )
    conn.commit()
    return conn


@pytest.fixture
def temp_db_with_data(temp_db: sqlite3.Connection) -> sqlite3.Connection:
    """Create a temporary database with sample model data."""
    models = [
        {
            "id": "diffusion/stability/sdxl-base",
            "path": "diffusion/stability/sdxl-base",
            "cleaned_name": "sdxl-base",
            "official_name": "SDXL Base 1.0",
            "model_type": "diffusion",
            "tags_json": '["checkpoint", "sd-xl"]',
            "hashes_json": '{"blake3": "abc123"}',
            "metadata_json": '{"family": "stability", "description": "Stable Diffusion XL"}',
            "updated_at": "2026-01-10T12:00:00Z",
        },
        {
            "id": "llm/meta/llama-3-8b",
            "path": "llm/meta/llama-3-8b",
            "cleaned_name": "llama-3-8b",
            "official_name": "Llama 3 8B",
            "model_type": "llm",
            "tags_json": '["gguf", "instruct"]',
            "hashes_json": '{"blake3": "def456"}',
            "metadata_json": '{"family": "meta", "description": "Meta Llama 3 8B parameter model"}',
            "updated_at": "2026-01-10T12:00:00Z",
        },
        {
            "id": "diffusion/runwayml/sd-v1-5",
            "path": "diffusion/runwayml/sd-v1-5",
            "cleaned_name": "sd-v1-5",
            "official_name": "Stable Diffusion v1.5",
            "model_type": "diffusion",
            "tags_json": '["checkpoint", "sd-1.5"]',
            "hashes_json": '{"blake3": "ghi789"}',
            "metadata_json": '{"family": "runwayml", "description": "Original SD 1.5"}',
            "updated_at": "2026-01-10T12:00:00Z",
        },
    ]

    for model in models:
        temp_db.execute(
            """
            INSERT INTO models (
                id, path, cleaned_name, official_name, model_type,
                tags_json, hashes_json, metadata_json, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            """,
            (
                model["id"],
                model["path"],
                model["cleaned_name"],
                model["official_name"],
                model["model_type"],
                model["tags_json"],
                model["hashes_json"],
                model["metadata_json"],
                model["updated_at"],
            ),
        )
    temp_db.commit()
    return temp_db


@pytest.mark.unit
class TestFTS5Config:
    """Tests for FTS5Config dataclass."""

    def test_default_config(self):
        """Test default FTS5 configuration values."""
        config = FTS5Config()
        assert config.table_name == "model_search"
        assert config.tokenizer == "unicode61"
        assert config.remove_diacritics is True
        assert config.tokenchars == "-_."
        assert "id" in config.indexed_columns
        assert "official_name" in config.indexed_columns

    def test_custom_config(self):
        """Test custom FTS5 configuration."""
        config = FTS5Config(
            table_name="custom_search",
            tokenizer="porter",
            remove_diacritics=False,
            tokenchars="",
        )
        assert config.table_name == "custom_search"
        assert config.tokenizer == "porter"
        assert config.remove_diacritics is False
        assert config.tokenchars == ""

    def test_tokenizer_options_string(self):
        """Test tokenizer options string generation."""
        config = FTS5Config()
        options = config.get_tokenizer_options()
        assert "unicode61" in options
        assert "remove_diacritics 1" in options

    def test_tokenizer_options_no_diacritics(self):
        """Test tokenizer options without diacritics removal."""
        config = FTS5Config(remove_diacritics=False)
        options = config.get_tokenizer_options()
        assert "remove_diacritics 0" in options


@pytest.mark.unit
class TestFTS5TableExists:
    """Tests for fts5_table_exists function."""

    def test_table_does_not_exist(self, temp_db: sqlite3.Connection):
        """Test detection of non-existent FTS5 table."""
        assert fts5_table_exists(temp_db) is False

    def test_table_exists(self, temp_db: sqlite3.Connection):
        """Test detection of existing FTS5 table."""
        temp_db.execute(
            """
            CREATE VIRTUAL TABLE model_search USING fts5(
                id, official_name, cleaned_name
            )
        """
        )
        temp_db.commit()
        assert fts5_table_exists(temp_db) is True

    def test_custom_table_name(self, temp_db: sqlite3.Connection):
        """Test detection with custom table name."""
        temp_db.execute(
            """
            CREATE VIRTUAL TABLE custom_search USING fts5(id, name)
        """
        )
        temp_db.commit()
        assert fts5_table_exists(temp_db, table_name="custom_search") is True
        assert fts5_table_exists(temp_db, table_name="model_search") is False


@pytest.mark.unit
class TestCreateFTS5Table:
    """Tests for create_fts5_table function."""

    def test_create_default_table(self, temp_db: sqlite3.Connection):
        """Test creating FTS5 table with default config."""
        create_fts5_table(temp_db)
        assert fts5_table_exists(temp_db) is True

        # Verify columns exist by inserting data
        temp_db.execute(
            """
            INSERT INTO model_search (
                id, official_name, cleaned_name, model_type, tags, family, description
            ) VALUES ('test', 'Test Model', 'test-model', 'llm', 'tag1', 'fam', 'desc')
        """
        )
        temp_db.commit()

    def test_create_custom_table(self, temp_db: sqlite3.Connection):
        """Test creating FTS5 table with custom config."""
        config = FTS5Config(table_name="custom_fts")
        create_fts5_table(temp_db, config=config)
        assert fts5_table_exists(temp_db, table_name="custom_fts") is True

    def test_idempotent_creation(self, temp_db: sqlite3.Connection):
        """Test that creating table twice doesn't fail."""
        create_fts5_table(temp_db)
        # Second call should not raise
        create_fts5_table(temp_db)
        assert fts5_table_exists(temp_db) is True


@pytest.mark.unit
class TestCreateFTS5Triggers:
    """Tests for create_fts5_triggers function."""

    def test_create_insert_trigger(self, temp_db_with_data: sqlite3.Connection):
        """Test that INSERT trigger syncs to FTS5."""
        create_fts5_table(temp_db_with_data)
        create_fts5_triggers(temp_db_with_data)

        # Insert a new model
        temp_db_with_data.execute(
            """
            INSERT INTO models (
                id, path, cleaned_name, official_name, model_type,
                tags_json, hashes_json, metadata_json, updated_at
            ) VALUES (
                'test/new/model', 'test/new/model', 'new-model', 'New Model',
                'llm', '["test"]', '{}', '{"family": "test", "description": "Test desc"}',
                '2026-01-10T12:00:00Z'
            )
        """
        )
        temp_db_with_data.commit()

        # Verify it's in FTS5
        row = temp_db_with_data.execute(
            "SELECT * FROM model_search WHERE id = ?", ("test/new/model",)
        ).fetchone()
        assert row is not None
        assert row["official_name"] == "New Model"

    def test_create_update_trigger(self, temp_db_with_data: sqlite3.Connection):
        """Test that UPDATE trigger syncs to FTS5."""
        create_fts5_table(temp_db_with_data)
        populate_fts5_from_models(temp_db_with_data)
        create_fts5_triggers(temp_db_with_data)

        # Update an existing model
        temp_db_with_data.execute(
            """
            UPDATE models
            SET official_name = 'Updated SDXL Name'
            WHERE id = 'diffusion/stability/sdxl-base'
        """
        )
        temp_db_with_data.commit()

        # Verify FTS5 was updated
        row = temp_db_with_data.execute(
            "SELECT official_name FROM model_search WHERE id = ?",
            ("diffusion/stability/sdxl-base",),
        ).fetchone()
        assert row is not None
        assert row["official_name"] == "Updated SDXL Name"

    def test_create_delete_trigger(self, temp_db_with_data: sqlite3.Connection):
        """Test that DELETE trigger removes from FTS5."""
        create_fts5_table(temp_db_with_data)
        populate_fts5_from_models(temp_db_with_data)
        create_fts5_triggers(temp_db_with_data)

        # Delete a model
        temp_db_with_data.execute(
            "DELETE FROM models WHERE id = ?", ("diffusion/stability/sdxl-base",)
        )
        temp_db_with_data.commit()

        # Verify it's removed from FTS5
        row = temp_db_with_data.execute(
            "SELECT * FROM model_search WHERE id = ?",
            ("diffusion/stability/sdxl-base",),
        ).fetchone()
        assert row is None


@pytest.mark.unit
class TestDropFTS5Triggers:
    """Tests for drop_fts5_triggers function."""

    def test_drop_triggers(self, temp_db_with_data: sqlite3.Connection):
        """Test dropping FTS5 triggers."""
        create_fts5_table(temp_db_with_data)
        create_fts5_triggers(temp_db_with_data)

        # Verify triggers exist
        triggers = temp_db_with_data.execute(
            """
            SELECT name FROM sqlite_master WHERE type = 'trigger'
        """
        ).fetchall()
        assert len(triggers) >= 3

        # Drop triggers
        drop_fts5_triggers(temp_db_with_data)

        # Verify triggers are gone
        triggers = temp_db_with_data.execute(
            """
            SELECT name FROM sqlite_master WHERE type = 'trigger'
        """
        ).fetchall()
        assert len(triggers) == 0


@pytest.mark.unit
class TestPopulateFTS5FromModels:
    """Tests for populate_fts5_from_models function."""

    def test_populate_empty_models(self, temp_db: sqlite3.Connection):
        """Test populating FTS5 from empty models table."""
        create_fts5_table(temp_db)
        count = populate_fts5_from_models(temp_db)
        assert count == 0

    def test_populate_with_data(self, temp_db_with_data: sqlite3.Connection):
        """Test populating FTS5 from models table with data."""
        create_fts5_table(temp_db_with_data)
        count = populate_fts5_from_models(temp_db_with_data)
        assert count == 3

        # Verify data is in FTS5
        rows = temp_db_with_data.execute("SELECT COUNT(*) as cnt FROM model_search").fetchone()
        assert rows["cnt"] == 3

    def test_populate_clears_existing(self, temp_db_with_data: sqlite3.Connection):
        """Test that populate clears existing FTS5 data first."""
        create_fts5_table(temp_db_with_data)

        # Populate once
        populate_fts5_from_models(temp_db_with_data)

        # Populate again - should not duplicate
        populate_fts5_from_models(temp_db_with_data)

        rows = temp_db_with_data.execute("SELECT COUNT(*) as cnt FROM model_search").fetchone()
        assert rows["cnt"] == 3


@pytest.mark.unit
class TestMigrateToFTS5:
    """Tests for migrate_to_fts5 function."""

    def test_migrate_creates_table(self, temp_db_with_data: sqlite3.Connection):
        """Test that migration creates FTS5 table."""
        assert fts5_table_exists(temp_db_with_data) is False
        migrate_to_fts5(temp_db_with_data)
        assert fts5_table_exists(temp_db_with_data) is True

    def test_migrate_populates_data(self, temp_db_with_data: sqlite3.Connection):
        """Test that migration populates FTS5 with existing data."""
        migrate_to_fts5(temp_db_with_data)

        rows = temp_db_with_data.execute("SELECT COUNT(*) as cnt FROM model_search").fetchone()
        assert rows["cnt"] == 3

    def test_migrate_creates_triggers(self, temp_db_with_data: sqlite3.Connection):
        """Test that migration creates sync triggers."""
        migrate_to_fts5(temp_db_with_data)

        # Insert new model and verify FTS5 sync
        temp_db_with_data.execute(
            """
            INSERT INTO models (
                id, path, cleaned_name, official_name, model_type,
                tags_json, hashes_json, metadata_json, updated_at
            ) VALUES (
                'test/after/migrate', 'test/after/migrate', 'after-migrate',
                'After Migration', 'llm', '[]', '{}',
                '{"family": "test", "description": "Post migration"}',
                '2026-01-10T12:00:00Z'
            )
        """
        )
        temp_db_with_data.commit()

        row = temp_db_with_data.execute(
            "SELECT * FROM model_search WHERE id = ?", ("test/after/migrate",)
        ).fetchone()
        assert row is not None

    def test_migrate_idempotent(self, temp_db_with_data: sqlite3.Connection):
        """Test that migration is idempotent."""
        migrate_to_fts5(temp_db_with_data)
        migrate_to_fts5(temp_db_with_data)  # Should not raise

        rows = temp_db_with_data.execute("SELECT COUNT(*) as cnt FROM model_search").fetchone()
        assert rows["cnt"] == 3


@pytest.mark.unit
class TestFTS5Manager:
    """Tests for FTS5Manager class."""

    def test_init_migrates_if_needed(self, temp_db_with_data: sqlite3.Connection):
        """Test that manager migrates on init if needed."""
        manager = FTS5Manager(temp_db_with_data)
        assert fts5_table_exists(temp_db_with_data) is True

    def test_rebuild(self, temp_db_with_data: sqlite3.Connection):
        """Test FTS5 rebuild functionality."""
        manager = FTS5Manager(temp_db_with_data)

        # Manually corrupt FTS5 by deleting data
        temp_db_with_data.execute("DELETE FROM model_search")
        temp_db_with_data.commit()

        # Rebuild should repopulate
        count = manager.rebuild()
        assert count == 3

        rows = temp_db_with_data.execute("SELECT COUNT(*) as cnt FROM model_search").fetchone()
        assert rows["cnt"] == 3

    def test_optimize(self, temp_db_with_data: sqlite3.Connection):
        """Test FTS5 optimize functionality."""
        manager = FTS5Manager(temp_db_with_data)
        # Should not raise
        manager.optimize()

    def test_get_stats(self, temp_db_with_data: sqlite3.Connection):
        """Test getting FTS5 statistics."""
        manager = FTS5Manager(temp_db_with_data)
        stats = manager.get_stats()

        assert "row_count" in stats
        assert stats["row_count"] == 3
        assert "table_name" in stats
        assert stats["table_name"] == "model_search"


@pytest.mark.unit
class TestFTS5ExtractsJsonFields:
    """Tests for JSON field extraction in FTS5."""

    def test_extracts_family_from_metadata(self, temp_db_with_data: sqlite3.Connection):
        """Test that family is extracted from metadata_json."""
        create_fts5_table(temp_db_with_data)
        populate_fts5_from_models(temp_db_with_data)

        row = temp_db_with_data.execute(
            "SELECT family FROM model_search WHERE id = ?",
            ("diffusion/stability/sdxl-base",),
        ).fetchone()
        assert row is not None
        assert row["family"] == "stability"

    def test_extracts_description_from_metadata(self, temp_db_with_data: sqlite3.Connection):
        """Test that description is extracted from metadata_json."""
        create_fts5_table(temp_db_with_data)
        populate_fts5_from_models(temp_db_with_data)

        row = temp_db_with_data.execute(
            "SELECT description FROM model_search WHERE id = ?",
            ("llm/meta/llama-3-8b",),
        ).fetchone()
        assert row is not None
        assert "Meta Llama 3" in row["description"]

    def test_extracts_tags_from_tags_json(self, temp_db_with_data: sqlite3.Connection):
        """Test that tags are extracted from tags_json."""
        create_fts5_table(temp_db_with_data)
        populate_fts5_from_models(temp_db_with_data)

        row = temp_db_with_data.execute(
            "SELECT tags FROM model_search WHERE id = ?",
            ("diffusion/stability/sdxl-base",),
        ).fetchone()
        assert row is not None
        assert "checkpoint" in row["tags"]
        assert "sd-xl" in row["tags"]
