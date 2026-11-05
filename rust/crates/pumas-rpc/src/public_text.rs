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
///
/// Control characters become spaces before redaction, so a credential split
/// from its label by a newline or tab (`Bearer\nX`) is still recognised.
pub(crate) fn public_reason_within(text: &str, max_chars: usize) -> String {
    let normalized: String = text
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    let mut text = redact_credentials(&normalized);
    if let Some(home) = std::env::var_os("HOME").filter(|home| home.len() > 1) {
        text = text.replace(home.to_string_lossy().as_ref(), "~");
    }
    let mut reason: String = text.chars().take(max_chars).collect();
    if text.chars().count() > max_chars {
        reason.pop();
        reason.push('…');
    }
    reason
}

/// Label names whose value is a credential (`token=X`, `token: X`,
/// `"token":"X"`). Matched case-insensitively as a suffix of the label.
const SECRET_KEYS: [&str; 7] = [
    "token",
    "key",
    "secret",
    "password",
    "api_key",
    "apikey",
    "authorization",
];

/// HTTP authorization schemes whose next word is the credential.
const AUTH_SCHEMES: [&str; 2] = ["bearer", "basic"];

fn is_secret_label(name: &str) -> bool {
    let name = name
        .trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '-')
        .to_ascii_lowercase();
    !name.is_empty() && SECRET_KEYS.iter().any(|key| name.ends_with(key))
}

/// Replace a credential value but keep the JSON/quote punctuation around it.
/// A quoted value ends at its closing quote; the rest of the word (for example
/// further JSON pairs) is redacted on its own merits.
fn redacted_value(word: &str) -> String {
    let opening = |c: char| matches!(c, '"' | '\'' | '{' | '[' | '(');
    let closing = |c: char| matches!(c, '"' | '\'' | '}' | ']' | ')' | ',' | ';');
    let start = word.len() - word.trim_start_matches(opening).len();
    if let Some(quote) = word[..start]
        .chars()
        .last()
        .filter(|c| matches!(c, '"' | '\''))
    {
        if let Some(close) = word[start..].find(quote).map(|at| start + at) {
            let rest = &word[close..];
            let mut redact_next = false;
            let rest = match rest.char_indices().nth(1) {
                Some((at, _)) => format!(
                    "{}{}",
                    &rest[..at],
                    redact_word(&rest[at..], &mut redact_next)
                ),
                None => rest.to_owned(),
            };
            return format!("{}[redacted]{rest}", &word[..start]);
        }
    }
    let end = word.trim_end_matches(closing).len().max(start);
    format!("{}[redacted]{}", &word[..start], &word[end..])
}

/// Replace credential-shaped words. A provider or library error may echo a
/// token from its own configuration. Words are separated by any whitespace,
/// and the separators are kept.
pub(crate) fn redact_credentials(text: &str) -> String {
    let mut redact_next = false;
    let mut output = String::with_capacity(text.len());
    let mut rest = text;
    while !rest.is_empty() {
        let word_end = rest.find(char::is_whitespace).unwrap_or(rest.len());
        let (word, tail) = rest.split_at(word_end);
        let separator_end = tail
            .find(|c: char| !c.is_whitespace())
            .unwrap_or(tail.len());
        let (separator, tail) = tail.split_at(separator_end);
        if !word.is_empty() {
            output.push_str(&redact_word(word, &mut redact_next));
        }
        output.push_str(separator);
        rest = tail;
    }
    output
}

fn redact_word(word: &str, redact_next: &mut bool) -> String {
    let bare = word.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '-');
    // `Authorization: Bearer X` keeps the scheme visible and hides X.
    if AUTH_SCHEMES
        .iter()
        .any(|scheme| bare.eq_ignore_ascii_case(scheme))
    {
        *redact_next = true;
        return word.to_owned();
    }
    if std::mem::take(redact_next) {
        return redacted_value(word);
    }
    // `token:` / `"token":` labels a value in the next word.
    if word.ends_with(':') && is_secret_label(word.trim_end_matches(':')) {
        *redact_next = true;
        return word.to_owned();
    }
    let credential_prefix = ["hf_", "sk-", "ghp_", "github_pat_", "xoxb-"]
        .iter()
        .any(|prefix| bare.starts_with(prefix) && bare.len() >= prefix.len() + 6);
    if credential_prefix {
        return word.replace(bare, "[redacted]");
    }
    if let Some(scheme) = word.find("://") {
        return redact_url(word, scheme);
    }
    // `token=X`, `token:X` and `"token":"X"` in one word.
    if let Some(at) = word.find(['=', ':']) {
        if is_secret_label(&word[..at]) {
            return format!("{}{}", &word[..=at], redacted_value(&word[at + 1..]));
        }
    }
    word.to_owned()
}

fn redact_url(word: &str, scheme: usize) -> String {
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

    #[test]
    fn labelled_and_json_credentials_are_redacted() {
        assert_eq!(
            public_reason("provider said token: abc123 is invalid"),
            "provider said token: [redacted] is invalid"
        );
        assert_eq!(
            public_reason(r#"body {"token":"abc123","model":"m"}"#),
            r#"body {"token":"[redacted]","model":"m"}"#
        );
        assert_eq!(
            public_reason(r#"body {"api_key": "abc123"}"#),
            r#"body {"api_key": "[redacted]"}"#
        );
        assert_eq!(
            public_reason("password:hunter2 rejected"),
            "password:[redacted] rejected"
        );
        assert_eq!(
            public_reason("Authorization: Basic QWxhZGRpbjpvcGVuIHNlc2FtZQ== refused"),
            "Authorization: Basic [redacted] refused"
        );
        assert_eq!(
            public_reason("Authorization: Bearer abc123 refused"),
            "Authorization: Bearer [redacted] refused"
        );
        assert_eq!(
            public_reason("Authorization: abc123 refused"),
            "Authorization: [redacted] refused"
        );
    }

    #[test]
    fn credentials_split_by_control_characters_or_whitespace_are_redacted() {
        assert_eq!(
            public_reason("auth Bearer\nabc123"),
            "auth Bearer [redacted]"
        );
        assert_eq!(
            public_reason("auth Bearer\tabc123"),
            "auth Bearer [redacted]"
        );
        assert_eq!(
            public_reason("auth Bearer\u{2003}abc123"),
            "auth Bearer\u{2003}[redacted]"
        );
        assert_eq!(public_reason("token:\r\nabc123"), "token:  [redacted]");
        assert_eq!(
            redact_credentials("Bearer\tabc123 ok"),
            "Bearer\t[redacted] ok"
        );
    }

    #[test]
    fn non_credential_labels_and_urls_are_kept() {
        assert_eq!(
            public_reason("max_tokens: 4096 exceeds context"),
            "max_tokens: 4096 exceeds context"
        );
        assert_eq!(
            public_reason("unknown field `foo`, expected one of `model`, `input`"),
            "unknown field `foo`, expected one of `model`, `input`"
        );
        assert_eq!(
            public_reason("connect http://127.0.0.1:8080/v1 failed"),
            "connect http://127.0.0.1:8080/v1 failed"
        );
    }
}
