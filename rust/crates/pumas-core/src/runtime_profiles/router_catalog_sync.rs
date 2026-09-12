//! Additive live preset reconciliation. Existing sections are immutable while
//! external autoload is enabled: an unloaded sample cannot exclude a later load.
use crate::{PumasError, Result};
use std::collections::BTreeMap;

pub(super) fn sections(bytes: &[u8]) -> Result<BTreeMap<String, String>> {
    let text = std::str::from_utf8(bytes).map_err(|_| error("Router preset is not UTF-8"))?;
    let mut sections = BTreeMap::new();
    let mut name = String::new();
    let mut body = String::new();
    for line in text.split_inclusive('\n') {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            if sections.insert(name, body).is_some() {
                return Err(error("Duplicate router preset section"));
            }
            name = trimmed[1..trimmed.len() - 1].to_string();
            body = String::new();
        }
        body.push_str(line);
    }
    if sections.insert(name, body).is_some() {
        return Err(error("Duplicate router preset section"));
    }
    Ok(sections)
}

pub(super) fn model_path(section: &str) -> Option<&str> {
    section
        .lines()
        .filter_map(|line| line.split_once('='))
        .find(|(key, _)| key.trim() == "model")
        .map(|(_, value)| value.trim())
}

/// Keep exact existing bytes, including custom context. Changes and removals
/// remain pending until a new process generation receives a new launch preset.
pub(super) fn merge_additions(current: &[u8], desired: &[u8]) -> Result<(Vec<u8>, Vec<String>)> {
    let existing = sections(current)?;
    let wanted = sections(desired)?;
    let mut pending = Vec::new();
    let mut output = current.to_vec();
    for (id, body) in &existing {
        if id.is_empty() || id == "*" {
            continue;
        }
        match wanted.get(id) {
            Some(next) if catalog_keys(body) == catalog_keys(next) => {}
            _ => pending.push(id.clone()),
        }
    }
    for (id, body) in wanted {
        if id.is_empty() || id == "*" || existing.contains_key(&id) {
            continue;
        }
        if !output.ends_with(b"\n") {
            output.push(b'\n');
        }
        output.extend_from_slice(body.as_bytes());
    }
    Ok((output, pending))
}
fn catalog_keys(section: &str) -> BTreeMap<&str, &str> {
    section
        .lines()
        .filter_map(|line| line.split_once('='))
        .map(|(k, v)| (k.trim(), v.trim()))
        .filter(|(k, _)| matches!(*k, "model" | "alias" | "embedding" | "reranking"))
        .collect()
}
fn error(message: &str) -> PumasError {
    PumasError::Other(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn additive_reconciliation_cannot_unload_a_child_that_started_after_sampling() {
        let current = b"version = 1\n[*]\nload-on-startup = false\n[a]\nmodel = /old\nctx-size = 18000\n[b]\nmodel = /b\n";
        let desired = b"version = 1\n[*]\nload-on-startup = false\n[a]\nmodel = /replacement\n[c]\nmodel = /c\n";
        let (merged, pending) = merge_additions(current, desired).unwrap();
        assert!(merged.starts_with(current));
        assert_eq!(pending, vec!["a", "b"]);
        assert!(String::from_utf8(merged.clone())
            .unwrap()
            .ends_with("[c]\nmodel = /c\n"));
        assert_eq!(merge_additions(&merged, desired).unwrap().0, merged);
    }
}
