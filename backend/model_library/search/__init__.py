"""Search operations for model library."""  # pragma: no cover

from __future__ import annotations  # pragma: no cover

from backend.model_library.search.fts5 import (  # pragma: no cover
    FTS5Config,
    FTS5Manager,
    create_fts5_table,
    create_fts5_triggers,
    drop_fts5_triggers,
    fts5_table_exists,
    migrate_to_fts5,
    populate_fts5_from_models,
)
from backend.model_library.search.query import (  # pragma: no cover
    SearchQuery,
    SearchResult,
    build_fts5_query,
    escape_fts5_term,
    search_models,
)

__all__ = [  # pragma: no cover
    # FTS5 Setup
    "FTS5Config",
    "FTS5Manager",
    "create_fts5_table",
    "create_fts5_triggers",
    "drop_fts5_triggers",
    "fts5_table_exists",
    "migrate_to_fts5",
    "populate_fts5_from_models",
    # Query Building
    "SearchQuery",
    "SearchResult",
    "build_fts5_query",
    "escape_fts5_term",
    "search_models",
]
