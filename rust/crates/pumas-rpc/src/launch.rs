//! Launch identity a parent supplies so it can tell its own child from any
//! other process answering on the same port.
//!
//! The desktop passes a fresh nonce in `PUMAS_RPC_LAUNCH_NONCE` (never argv,
//! which other local users can read). It is removed from the environment
//! before any thread or child process starts, reported once in the
//! `PUMAS_RPC_READY=` stdout line and echoed by `health_check`.

use std::sync::OnceLock;

pub(crate) const LAUNCH_NONCE_ENV: &str = "PUMAS_RPC_LAUNCH_NONCE";
const MAX_NONCE_BYTES: usize = 256;

static LAUNCH_NONCE: OnceLock<Option<String>> = OnceLock::new();

/// Validate a supplied nonce: 1 to 256 visible ASCII characters.
pub(crate) fn parse_nonce(value: Option<String>) -> anyhow::Result<Option<String>> {
    let Some(value) = value else {
        return Ok(None);
    };
    if value.is_empty()
        || value.len() > MAX_NONCE_BYTES
        || !value.bytes().all(|byte| byte.is_ascii_graphic())
    {
        anyhow::bail!("{LAUNCH_NONCE_ENV} must be 1 to {MAX_NONCE_BYTES} visible ASCII characters");
    }
    Ok(Some(value))
}

/// Take the nonce out of the environment. Call before any thread starts.
pub(crate) fn take_from_env() -> anyhow::Result<()> {
    let value = std::env::var_os(LAUNCH_NONCE_ENV)
        .map(|value| {
            value
                .into_string()
                .map_err(|_| anyhow::anyhow!("{LAUNCH_NONCE_ENV} must be ASCII"))
        })
        .transpose()?;
    // Runtime children (llama.cpp, Ollama, Torch) must not inherit it.
    std::env::remove_var(LAUNCH_NONCE_ENV);
    let nonce = parse_nonce(value)?;
    let _ = LAUNCH_NONCE.set(nonce);
    Ok(())
}

/// The nonce this process was launched with, if any.
pub(crate) fn nonce() -> Option<&'static str> {
    LAUNCH_NONCE.get().and_then(Option::as_deref)
}

/// The structured readiness line written after the listener is bound.
pub(crate) fn ready_line(port: u16, nonce: Option<&str>) -> String {
    format!(
        "PUMAS_RPC_READY={}",
        serde_json::json!({"version": 1, "port": port, "nonce": nonce})
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nonce_is_bounded_visible_ascii() {
        assert_eq!(parse_nonce(None).unwrap(), None);
        assert_eq!(
            parse_nonce(Some("a1-B2_c3".into())).unwrap().as_deref(),
            Some("a1-B2_c3")
        );
        for bad in ["", "has space", "tab\t", &"x".repeat(257), "é"] {
            let error = parse_nonce(Some(bad.into())).unwrap_err().to_string();
            assert!(error.contains(LAUNCH_NONCE_ENV), "{bad:?}: {error}");
        }
    }

    #[test]
    fn ready_line_is_one_json_object() {
        let line = ready_line(4321, Some("n0nce"));
        let json = line.strip_prefix("PUMAS_RPC_READY=").unwrap();
        let value: serde_json::Value = serde_json::from_str(json).unwrap();
        assert_eq!(
            value,
            serde_json::json!({"version": 1, "port": 4321, "nonce": "n0nce"})
        );
        assert!(!line.contains('\n'));
        let value: serde_json::Value = serde_json::from_str(
            ready_line(1, None)
                .strip_prefix("PUMAS_RPC_READY=")
                .unwrap(),
        )
        .unwrap();
        assert!(value["nonce"].is_null());
    }
}
