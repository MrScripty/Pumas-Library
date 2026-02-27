//! Ollama model naming helpers.

/// Derive an Ollama-friendly model name from a library display name.
///
/// Lowercases, replaces spaces and special characters with hyphens,
/// and collapses consecutive hyphens.
pub fn derive_ollama_name(display_name: &str) -> String {
    let name: String = display_name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();

    // Collapse consecutive hyphens and trim.
    let mut result = String::with_capacity(name.len());
    let mut last_was_hyphen = false;
    for c in name.chars() {
        if c == '-' {
            if !last_was_hyphen && !result.is_empty() {
                result.push('-');
            }
            last_was_hyphen = true;
        } else {
            result.push(c);
            last_was_hyphen = false;
        }
    }

    result.trim_end_matches('-').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_derive_ollama_name() {
        assert_eq!(derive_ollama_name("Llama 2 7B"), "llama-2-7b");
        assert_eq!(derive_ollama_name("Mistral 7B Q4_K_M"), "mistral-7b-q4_k_m");
        assert_eq!(derive_ollama_name("my-model"), "my-model");
        assert_eq!(
            derive_ollama_name("Model  With   Spaces"),
            "model-with-spaces"
        );
        assert_eq!(derive_ollama_name("model.v2"), "model.v2");
    }
}
