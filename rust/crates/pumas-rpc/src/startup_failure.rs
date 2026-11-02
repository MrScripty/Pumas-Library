//! Startup diagnostics consumed by the desktop bridge.
//!
//! Every fatal startup error writes one `PUMAS_STARTUP_FAILURE=` line holding a
//! [`StartupFailureFrame`], so the desktop shows what went wrong instead of
//! waiting for readiness. The frame's schema is exported in the desktop
//! contract and decoded there by the generated validator.
use pumas_library::PumasError;
use serde::Serialize;
use std::io::Write;

pub(crate) const STARTUP_FAILURE_PREFIX: &str = "PUMAS_STARTUP_FAILURE=";
/// Keeps the whole line below the desktop reader's 1,024-character bound.
const MAX_DETAIL_CHARS: usize = 700;

/// Why the backend could not start.
///
/// The desktop may run a backend from another build (for example a developer
/// build against an installed app's library), so this frame is read across
/// versions: readers ignore unknown fields and treat an unknown `reason` as
/// `backend-unavailable`.
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
#[derive(Debug, Serialize, PartialEq, Eq)]
pub(crate) struct StartupFailureFrame {
    /// Frame format version; currently 1.
    pub(crate) version: u32,
    /// `migration-required` (the library needs the metadata upgrade flow) or
    /// `backend-unavailable` (any other startup failure).
    #[cfg_attr(feature = "export-contract", schemars(length(min = 1, max = 64)))]
    pub(crate) reason: String,
    /// Human-readable explanation, bounded, with the home directory shown as
    /// `~`. Absent when the reason has its own explanation in the desktop.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "export-contract", schemars(length(min = 1, max = 1024)))]
    pub(crate) detail: Option<String>,
}

pub(crate) const MIGRATION_REQUIRED: &str = "migration-required";
pub(crate) const BACKEND_UNAVAILABLE: &str = "backend-unavailable";

impl StartupFailureFrame {
    pub(crate) fn for_error(error: &anyhow::Error) -> Self {
        let migration_required = error.chain().any(|cause| {
            matches!(cause.downcast_ref::<PumasError>(), Some(PumasError::Validation { field, .. })
                if field == "acquisition.migration_required")
        });
        if migration_required {
            // The upgrade flow has its own explanation and action.
            Self {
                version: 1,
                reason: MIGRATION_REQUIRED.into(),
                detail: None,
            }
        } else {
            Self {
                version: 1,
                reason: BACKEND_UNAVAILABLE.into(),
                detail: Some(detail(error)),
            }
        }
    }
}

pub(crate) fn write(error: &anyhow::Error, output: &mut impl Write) -> std::io::Result<()> {
    let frame = serde_json::to_string(&StartupFailureFrame::for_error(error))
        .map_err(std::io::Error::other)?;
    writeln!(output, "{STARTUP_FAILURE_PREFIX}{frame}")?;
    output.flush()
}

fn detail(error: &anyhow::Error) -> String {
    let mut text = error
        .chain()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(": ");
    if let Some(home) = std::env::var_os("HOME").filter(|home| !home.is_empty()) {
        text = text.replace(home.to_string_lossy().as_ref(), "~");
    }
    let mut detail: String = text
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .take(MAX_DETAIL_CHARS)
        .collect();
    if text.chars().count() > MAX_DETAIL_CHARS {
        detail.push('…');
    }
    if detail.trim().is_empty() {
        detail = "The backend stopped before it was ready.".into();
    }
    detail
}

#[cfg(test)]
mod tests {
    use super::*;

    fn written(error: &anyhow::Error) -> String {
        let mut output = Vec::new();
        write(error, &mut output).unwrap();
        String::from_utf8(output).unwrap()
    }

    #[test]
    fn migration_diagnostic_preserves_typed_failure_without_private_context() {
        let error = anyhow::Error::from(PumasError::Validation {
            field: "acquisition.migration_required".into(),
            message: "/private/library secret detail".into(),
        })
        .context("constructor context");
        // Byte-identical to the frame older desktop builds accept.
        assert_eq!(
            written(&error),
            "PUMAS_STARTUP_FAILURE={\"version\":1,\"reason\":\"migration-required\"}\n"
        );
    }

    #[test]
    fn diagnostic_does_not_infer_migration_from_error_text() {
        let frame =
            StartupFailureFrame::for_error(&anyhow::anyhow!("acquisition.migration_required"));
        assert_eq!(frame.reason, BACKEND_UNAVAILABLE);
    }

    #[test]
    fn other_startup_failures_explain_themselves_within_bounds() {
        let frame = StartupFailureFrame::for_error(
            &anyhow::anyhow!("unknown field `hf_model_partition_version`\nexpected one of")
                .context("Failed to open library"),
        );
        assert_eq!(frame.version, 1);
        assert_eq!(frame.reason, BACKEND_UNAVAILABLE);
        let detail = frame.detail.unwrap();
        assert!(detail.starts_with("Failed to open library: unknown field"));
        assert!(!detail.contains('\n'));

        let line = written(&anyhow::anyhow!("{}", "x".repeat(5000)));
        assert!(line.trim_end().chars().count() < 1024, "{}", line.len());
    }
}
