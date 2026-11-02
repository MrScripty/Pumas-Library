//! Human-readable failure text that leaves the process.
//!
//! Callers branch on stable codes; this text explains the failure to a person.
//! It keeps the reason but removes credentials, abbreviates the home directory
//! to `~`, replaces control characters and bounds the length.

/// Upper bound on a public failure explanation, in characters.
pub(crate) const MAX_PUBLIC_REASON_CHARS: usize = 1024;

/// Prepare `text` for a caller or the desktop.
pub(crate) fn public_reason(text: &str) -> String {
    public_reason_within(text, MAX_PUBLIC_REASON_CHARS)
}

/// [`public_reason`] with a caller-selected bound, for size-limited channels.
pub(crate) fn public_reason_within(text: &str, max_chars: usize) -> String {
    let mut text = redact_credentials(text);
    if let Some(home) = std::env::var_os("HOME").filter(|home| home.len() > 1) {
        text = text.replace(home.to_string_lossy().as_ref(), "~");
    }
    let mut reason: String = text
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .take(max_chars)
        .collect();
    if text.chars().count() > max_chars {
        reason.pop();
        reason.push('…');
    }
    reason
}

/// Replace credential-shaped words. A provider or library error may echo a
/// token from its own configuration.
pub(crate) fn redact_credentials(text: &str) -> String {
    const SECRET_KEYS: [&str; 6] = ["token", "key", "secret", "password", "api_key", "apikey"];
    let mut redact_next = false;
    text.split(' ')
        .map(|word| {
            if std::mem::take(&mut redact_next) && !word.is_empty() {
                return "[redacted]".to_owned();
            }
            let bare =
                word.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '-');
            if bare.eq_ignore_ascii_case("bearer") {
                redact_next = true;
                return word.to_owned();
            }
            let credential_prefix = ["hf_", "sk-", "ghp_", "github_pat_", "xoxb-"]
                .iter()
                .any(|prefix| bare.starts_with(prefix) && bare.len() >= prefix.len() + 6);
            if credential_prefix {
                return word.replace(bare, "[redacted]");
            }
            if let Some((name, _)) = word.split_once('=') {
                let name = name.trim_start_matches(|c: char| !c.is_ascii_alphanumeric());
                if SECRET_KEYS
                    .iter()
                    .any(|key| name.to_ascii_lowercase().ends_with(key))
                {
                    return format!("{name}=[redacted]");
                }
            }
            let Some(scheme) = word.find("://") else {
                return word.to_owned();
            };
            let mut word = word.to_owned();
            // A query can carry a signature (for example a presigned S3 URL).
            if let Some(query) = word[scheme..].find('?').map(|at| scheme + at) {
                word.truncate(query);
                word.push_str("?[redacted]");
            }
            // scheme://user:password@host
            match word[scheme..].find('@').map(|at| scheme + at) {
                Some(at) => format!("{}[redacted]{}", &word[..scheme + 3], &word[at..]),
                None => word,
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reasons_keep_context_but_not_credentials() {
        assert_eq!(
            redact_credentials("input is too large to process. increase the physical batch size"),
            "input is too large to process. increase the physical batch size"
        );
        assert_eq!(
            redact_credentials(
                "auth failed: Bearer abc.def token=xyz https://user:pw@host/p sk-abcdefghij"
            ),
            "auth failed: Bearer [redacted] token=[redacted] https://[redacted]@host/p [redacted]"
        );
        assert_eq!(
            redact_credentials(
                "GET https://bucket.s3.amazonaws.com/m.gguf?X-Amz-Signature=abc failed"
            ),
            "GET https://bucket.s3.amazonaws.com/m.gguf?[redacted] failed"
        );
    }

    #[test]
    fn reasons_are_single_line_and_bounded() {
        let reason = public_reason_within("first\nsecond", 100);
        assert_eq!(reason, "first second");
        let long = public_reason_within(&"x".repeat(50), 10);
        assert_eq!(long.chars().count(), 10);
        assert!(long.ends_with('…'));
    }
}
