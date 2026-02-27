//! FTS5 query building utilities.

use regex::Regex;
use std::sync::LazyLock;

/// Special characters that need escaping in FTS5 queries.
static FTS5_SPECIAL_CHARS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"[-._"]"#).unwrap());

/// Escape a term for FTS5 queries.
///
/// Terms containing special characters (hyphen, dot, underscore, quote) are wrapped in quotes.
pub fn escape_fts5_term(term: &str) -> String {
    if FTS5_SPECIAL_CHARS.is_match(term) {
        // Double any existing quotes and wrap in quotes
        let escaped = term.replace('"', "\"\"");
        format!("\"{}\"", escaped)
    } else {
        term.to_string()
    }
}

/// Build an FTS5 query string from a search term.
///
/// The query uses OR matching with prefix support:
/// - "gpt-2 model" → `"gpt-2"* OR model*`
/// - "stable.diffusion" → `"stable.diffusion"*`
/// - "vae_decoder" → `"vae_decoder"*`
pub fn build_fts5_query(search_term: &str) -> String {
    let search_term = search_term.to_lowercase().trim().to_string();

    if search_term.is_empty() {
        return String::new();
    }

    let terms: Vec<&str> = search_term.split_whitespace().collect();
    let mut query_parts = Vec::new();

    for term in terms {
        let escaped = escape_fts5_term(term);
        if !escaped.is_empty() {
            // Add prefix matching with *
            query_parts.push(format!("{}*", escaped));
        }
    }

    query_parts.join(" OR ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_simple_term() {
        assert_eq!(escape_fts5_term("model"), "model");
        assert_eq!(escape_fts5_term("llama"), "llama");
    }

    #[test]
    fn test_escape_hyphen() {
        assert_eq!(escape_fts5_term("gpt-2"), "\"gpt-2\"");
        assert_eq!(escape_fts5_term("stable-diffusion"), "\"stable-diffusion\"");
    }

    #[test]
    fn test_escape_dot() {
        assert_eq!(escape_fts5_term("v1.5"), "\"v1.5\"");
        assert_eq!(
            escape_fts5_term("model.safetensors"),
            "\"model.safetensors\""
        );
    }

    #[test]
    fn test_escape_underscore() {
        assert_eq!(escape_fts5_term("vae_decoder"), "\"vae_decoder\"");
    }

    #[test]
    fn test_escape_quotes() {
        assert_eq!(escape_fts5_term("test\"quote"), "\"test\"\"quote\"");
    }

    #[test]
    fn test_build_query_single_term() {
        assert_eq!(build_fts5_query("llama"), "llama*");
        assert_eq!(build_fts5_query("gpt-2"), "\"gpt-2\"*");
    }

    #[test]
    fn test_build_query_multiple_terms() {
        assert_eq!(build_fts5_query("llama model"), "llama* OR model*");
        assert_eq!(build_fts5_query("gpt-2 base"), "\"gpt-2\"* OR base*");
    }

    #[test]
    fn test_build_query_empty() {
        assert_eq!(build_fts5_query(""), "");
        assert_eq!(build_fts5_query("   "), "");
    }

    #[test]
    fn test_build_query_case_insensitive() {
        assert_eq!(build_fts5_query("LLAMA"), "llama*");
        assert_eq!(build_fts5_query("GPT-2"), "\"gpt-2\"*");
    }
}
