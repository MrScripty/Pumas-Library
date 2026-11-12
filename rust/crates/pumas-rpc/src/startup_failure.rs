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
    /// With `stale-instance`: the library's instance entry that no live owner
    /// can be shown to hold, which an operator may release.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) stale_instance: Option<StaleInstanceFrame>,
}

/// The entry an operator is asked to release, as `--release-stale-instance`
/// must be given it back.
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
#[derive(Debug, Serialize, PartialEq, Eq)]
pub(crate) struct StaleInstanceFrame {
    pub(crate) pid: u32,
    #[cfg_attr(feature = "export-contract", schemars(length(min = 1, max = 64)))]
    pub(crate) started_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "export-contract", schemars(length(min = 1, max = 64)))]
    pub(crate) version: Option<String>,
    /// Whether a free store lease backs the finding (false on Windows, where
    /// only the process check does).
    pub(crate) lease_checked: bool,
}

pub(crate) const MIGRATION_REQUIRED: &str = "migration-required";
pub(crate) const BACKEND_UNAVAILABLE: &str = "backend-unavailable";
pub(crate) const STALE_INSTANCE: &str = "stale-instance";

fn bounded(value: &str, max: usize) -> String {
    value.chars().take(max).collect()
}

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
                stale_instance: None,
            }
        } else {
            Self {
                version: 1,
                reason: BACKEND_UNAVAILABLE.into(),
                detail: Some(detail(error)),
                stale_instance: None,
            }
        }
    }

    /// Like [`Self::for_error`], but a failure other than a required
    /// migration is reported as `stale-instance` when the library's instance
    /// entry has no live owner, so the desktop can offer to release it.
    pub(crate) fn for_error_at(
        error: &anyhow::Error,
        stale: Option<&pumas_library::registry::StaleInstance>,
    ) -> Self {
        let mut frame = Self::for_error(error);
        if let (Some(stale), true) = (stale, frame.reason == BACKEND_UNAVAILABLE) {
            frame.reason = STALE_INSTANCE.into();
            frame.detail = Some(bounded(
                &format!(
                    "The library is registered as open by Pumas process {}, which is no longer running{}. Release its entry to open the library.",
                    stale.pid,
                    if stale.lease_checked {
                        " and holds no lock on the library"
                    } else {
                        ""
                    }
                ),
                MAX_DETAIL_CHARS,
            ));
            frame.stale_instance = Some(StaleInstanceFrame {
                pid: stale.pid,
                started_at: bounded(&stale.started_at, 64),
                version: stale.version.as_deref().map(|version| bounded(version, 64)),
                lease_checked: stale.lease_checked,
            });
        }
        frame
    }
}

#[cfg(test)]
pub(crate) fn write(error: &anyhow::Error, output: &mut impl Write) -> std::io::Result<()> {
    write_frame(&StartupFailureFrame::for_error(error), output)
}

/// Write the failure for `launcher_root`, naming a stale instance entry when
/// the library has one.
pub(crate) fn write_for_root(
    error: &anyhow::Error,
    launcher_root: &std::path::Path,
    output: &mut impl Write,
) -> std::io::Result<()> {
    let stale = pumas_library::registry::LibraryRegistry::open()
        .ok()
        .and_then(|registry| {
            registry
                .inspect_stale_instance(launcher_root)
                .ok()
                .flatten()
        });
    write_frame(
        &StartupFailureFrame::for_error_at(error, stale.as_ref()),
        output,
    )
}

fn write_frame(frame: &StartupFailureFrame, output: &mut impl Write) -> std::io::Result<()> {
    let frame = serde_json::to_string(frame).map_err(std::io::Error::other)?;
    writeln!(output, "{STARTUP_FAILURE_PREFIX}{frame}")?;
    output.flush()
}

fn detail(error: &anyhow::Error) -> String {
    let text = error
        .chain()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(": ");
    let detail = crate::public_text::public_reason_within(&text, MAX_DETAIL_CHARS);
    if detail.trim().is_empty() {
        "The backend stopped before it was ready.".into()
    } else {
        detail
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stale_instance_entry_is_named_so_the_desktop_can_offer_its_release() {
        let error =
            anyhow::anyhow!("Pumas library instance is already running for /library (pid 42)");
        let stale = pumas_library::registry::StaleInstance {
            library_path: "/library".into(),
            pid: 42,
            started_at: "2026-11-10T19:40:23Z".into(),
            version: Some("0.8.0".into()),
            lease_checked: true,
        };
        let frame = StartupFailureFrame::for_error_at(&error, Some(&stale));
        assert_eq!(frame.reason, STALE_INSTANCE);
        assert!(frame
            .detail
            .as_deref()
            .unwrap()
            .contains("process 42, which is no longer running"));
        assert_eq!(
            serde_json::to_value(&frame).unwrap()["stale_instance"],
            serde_json::json!({"pid": 42, "started_at": "2026-11-10T19:40:23Z", "version": "0.8.0", "lease_checked": true})
        );
        // A required migration keeps its own flow even with a stale entry.
        let migration = anyhow::Error::from(PumasError::Validation {
            field: "acquisition.migration_required".into(),
            message: "upgrade".into(),
        });
        let frame = StartupFailureFrame::for_error_at(&migration, Some(&stale));
        assert_eq!(
            (frame.reason.as_str(), frame.stale_instance),
            (MIGRATION_REQUIRED, None)
        );
        // Without a stale entry nothing changes.
        assert_eq!(
            StartupFailureFrame::for_error_at(&error, None),
            StartupFailureFrame::for_error(&error)
        );
        // The whole frame stays inside the desktop reader's line bound.
        let mut output = Vec::new();
        write_frame(
            &StartupFailureFrame::for_error_at(&error, Some(&stale)),
            &mut output,
        )
        .unwrap();
        assert!(output.len() < 1024, "{}", output.len());
    }

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
