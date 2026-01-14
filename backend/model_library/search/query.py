"""Search query builder for FTS5 full-text search.

Provides utilities for building FTS5 queries with prefix matching,
escaping special characters, and filtering search results.
"""

from __future__ import annotations

import json
import re
import sqlite3
import time
from dataclasses import dataclass, field
from typing import Any

from backend.logging_config import get_logger
from backend.model_library.related import has_related_metadata

logger = get_logger(__name__)

# Characters that need quoting in FTS5
FTS5_SPECIAL_CHARS = re.compile(r"[-._]")


def escape_fts5_term(term: str) -> str:
    """Escape a search term for FTS5 query.

    Handles special characters that have meaning in FTS5 queries
    by wrapping terms in double quotes when necessary.

    Args:
        term: Search term to escape

    Returns:
        Escaped term safe for FTS5 query
    """
    term = term.strip()
    if not term:
        return ""

    # If term contains special characters, quote it
    if FTS5_SPECIAL_CHARS.search(term):
        # Escape any existing double quotes by doubling them
        escaped = term.replace('"', '""')
        return f'"{escaped}"'

    return term


def build_fts5_query(search_term: str) -> str:
    """Build an FTS5 query string from user search input.

    Converts user input into an FTS5 MATCH query with prefix
    matching enabled. Multiple words are OR'd together.

    Args:
        search_term: User's search input

    Returns:
        FTS5 query string ready for MATCH clause
    """
    if not search_term or not search_term.strip():
        return ""

    # Normalize to lowercase
    search_term = search_term.lower().strip()

    # Split on whitespace
    terms = search_term.split()

    # Build query parts
    query_parts = []
    for term in terms:
        if not term:
            continue

        # Escape the term
        escaped = escape_fts5_term(term)
        if escaped:
            # Add prefix matching with *
            query_parts.append(f"{escaped}*")

    if not query_parts:
        return ""

    # Join with OR for inclusive search
    return " OR ".join(query_parts)


@dataclass
class SearchQuery:
    """Configuration for a model search query.

    Attributes:
        terms: Search terms (space-separated for OR matching)
        limit: Maximum number of results to return
        model_type: Filter by model type (e.g., "diffusion", "llm")
        tags: Filter by tags (all tags must match)
        offset: Number of results to skip (for pagination)
    """

    terms: str
    limit: int = 100
    model_type: str | list[str] | None = None
    tags: list[str] | None = None
    offset: int = 0


@dataclass
class SearchResult:
    """Result from a model search.

    Attributes:
        models: List of matching model metadata dictionaries
        total_count: Total number of matching models
        query_time_ms: Time taken to execute query in milliseconds
        query: The FTS5 query that was executed
    """

    models: list[dict[str, Any]]
    total_count: int
    query_time_ms: float
    query: str


def search_models(
    conn: sqlite3.Connection,
    terms: str,
    limit: int = 100,
    offset: int = 0,
    model_type: str | list[str] | None = None,
    tags: list[str] | None = None,
) -> SearchResult:
    """Search models using FTS5 full-text search.

    Performs a fast full-text search across model metadata including
    names, types, tags, family, and description.

    Args:
        conn: SQLite database connection with FTS5 table
        terms: Search terms (space-separated for OR matching)
        limit: Maximum number of results to return
        offset: Number of results to skip
        model_type: Filter by model type(s)
        tags: Filter by required tags

    Returns:
        SearchResult with matching models and statistics
    """
    start_time = time.perf_counter()

    # Build FTS5 query
    fts5_query = build_fts5_query(terms)

    # Start building SQL
    params: list[Any] = []

    if fts5_query:
        # Use FTS5 for search
        sql = """
            SELECT m.*, ms.rank
            FROM model_search ms
            JOIN models m ON ms.id = m.id
            WHERE model_search MATCH ?
        """
        params.append(fts5_query)
    else:
        # Empty query - return all models
        sql = """
            SELECT m.*, 0 as rank
            FROM models m
            WHERE 1=1
        """

    # Add model type filter
    if model_type:
        if isinstance(model_type, str):
            sql += " AND m.model_type = ?"
            params.append(model_type)
        else:
            placeholders = ", ".join("?" for _ in model_type)
            sql += f" AND m.model_type IN ({placeholders})"
            params.extend(model_type)

    # Add ordering
    if fts5_query:
        sql += " ORDER BY rank"
    else:
        sql += " ORDER BY m.updated_at DESC"

    # Add limit and offset
    sql += " LIMIT ? OFFSET ?"
    params.extend([limit, offset])

    # Execute query
    try:
        cursor = conn.execute(sql, params)
        rows = cursor.fetchall()
    except sqlite3.OperationalError as e:
        logger.error("FTS5 search failed: %s", e)
        # Fallback to empty results on error
        rows = []

    # Convert rows to dictionaries
    models = []
    for row in rows:
        model = dict(row)
        # Remove rank from output
        model.pop("rank", None)

        # Parse JSON fields
        try:
            model["tags"] = json.loads(model.get("tags_json", "[]"))
        except (
            json.JSONDecodeError,
            TypeError,
        ):  # noqa: multi-exception  # noqa: no-except-logging
            model["tags"] = []

        try:
            metadata = json.loads(model.get("metadata_json", "{}"))
            model["family"] = metadata.get("family", "")
            model["description"] = metadata.get("description", "")
            model["related_available"] = has_related_metadata(metadata)
        except (
            json.JSONDecodeError,
            TypeError,
        ):  # noqa: multi-exception  # noqa: no-except-logging
            model["family"] = ""
            model["description"] = ""
            model["related_available"] = False

        models.append(model)

    # Filter by tags if specified
    if tags:
        models = [
            m
            for m in models
            if all(tag.lower() in [t.lower() for t in m.get("tags", [])] for tag in tags)
        ]

    # Calculate query time
    query_time_ms = (time.perf_counter() - start_time) * 1000

    return SearchResult(
        models=models,
        total_count=len(models),
        query_time_ms=query_time_ms,
        query=fts5_query,
    )
