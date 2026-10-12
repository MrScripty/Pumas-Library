//! Path-free, versioned startup diagnostics consumed by the desktop bridge.
use pumas_library::PumasError;
use std::io::Write;

pub(crate) fn write(error: &anyhow::Error, output: &mut impl Write) -> std::io::Result<()> {
    if error.chain().any(|cause| {
        matches!(cause.downcast_ref::<PumasError>(), Some(PumasError::Validation { field, .. })
            if field == "acquisition.migration_required")
    }) {
        writeln!(
            output,
            "PUMAS_STARTUP_FAILURE={{\"version\":1,\"reason\":\"migration-required\"}}"
        )?;
        output.flush()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migration_diagnostic_preserves_typed_failure_without_private_context() {
        let error = anyhow::Error::from(PumasError::Validation {
            field: "acquisition.migration_required".into(),
            message: "/private/library secret detail".into(),
        })
        .context("constructor context");
        let mut output = Vec::new();
        write(&error, &mut output).unwrap();
        assert_eq!(
            String::from_utf8(output).unwrap(),
            "PUMAS_STARTUP_FAILURE={\"version\":1,\"reason\":\"migration-required\"}\n"
        );
    }

    #[test]
    fn diagnostic_does_not_infer_migration_from_error_text() {
        let mut output = Vec::new();
        write(
            &anyhow::anyhow!("acquisition.migration_required"),
            &mut output,
        )
        .unwrap();
        assert!(output.is_empty());
    }
}
