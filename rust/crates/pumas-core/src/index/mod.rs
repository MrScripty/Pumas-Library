//! SQLite model index with FTS5 full-text search.
//!
//! This module provides:
//! - Model metadata storage in SQLite
//! - FTS5 full-text search capabilities
//! - Query building and search execution

mod fts5;
mod model_index;
mod query;

pub use fts5::{FTS5Config, FTS5Manager};
pub use model_index::{
    DependencyBindingHistoryRecord, DependencyProfileRecord, ModelDependencyBindingRecord,
    ModelIndex, ModelRecord, ModelTypeArchRule, ModelTypeConfigRule, SearchResult,
    TaskSignatureMapping,
};
pub use query::{build_fts5_query, escape_fts5_term};
