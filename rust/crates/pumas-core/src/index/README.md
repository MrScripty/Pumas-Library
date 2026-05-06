# Index

## Purpose

SQLite-backed full-text search index for the model library. Uses SQLite FTS5 virtual tables
to enable fast, typo-tolerant searching across model names, types, and tags. Provides query
building utilities that handle FTS5 special character escaping and prefix matching.

## Contents

| File | Description |
|------|-------------|
| `mod.rs` | Module root, re-exports public API |
| `model_index.rs` | `ModelIndex` - SQLite storage for model records with insert, update, delete, search, package-fact cache, update feed, and selector snapshot projection |
| `fts5.rs` | `FTS5Config` / `FTS5Manager` - FTS5 virtual table setup, tokenizer configuration, maintenance |
| `query.rs` | `build_fts5_query` / `escape_fts5_term` - Query building with OR matching and prefix support |

## Design Decisions

- **FTS5 over FTS4/LIKE**: FTS5 provides better ranking (BM25), prefix queries, and lower memory
  usage than alternatives. The `unicode61` tokenizer with diacritic removal ensures broad
  compatibility with international model names.
- **Separate query builder**: FTS5 has its own query syntax with special characters that need
  escaping. Centralizing query construction prevents injection and escaping bugs.
- **Read-only index handle**: Snapshot-style readers can open an existing
  `models.db` with SQLite read-only flags and `PRAGMA query_only=ON` so they do
  not create schema or mutate indexed state.

## Dependencies

### Internal
- `crate::error` - `PumasError` / `Result`

### External
- `rusqlite` - SQLite database access with FTS5 extension
- `regex` - Special character detection in query builder
