//! Script observations share the native runner's cleanup ownership. Script
//! errors become operation failures only after that runner has drained work;
//! terminal progress remains the retained conversion worker's responsibility.

use serde::Deserialize;
use serde_json::Value;
use tokio::process::Command;
use tracing::debug;

use super::native_process::{self, OutputStream};
use super::progress::ConversionProgressTracker;
use super::types::ScriptProgressLine;
use crate::cancel::CancellationToken;
use crate::{PumasError, Result};

#[derive(Deserialize)]
struct Observation {
    #[serde(flatten)]
    progress: ScriptProgressLine,
    epoch: Option<Value>,
    epochs_total: Option<Value>,
}

fn epoch_pair(observation: &Observation) -> std::result::Result<Option<(u32, u32)>, String> {
    let (Some(epoch), Some(total)) = (&observation.epoch, &observation.epochs_total) else {
        return Ok(None);
    };
    let epoch = epoch.as_u64().and_then(|value| u32::try_from(value).ok());
    let total = total.as_u64().and_then(|value| u32::try_from(value).ok());
    match (epoch, total) {
        (Some(epoch), Some(total)) if epoch > 0 && epoch <= total => Ok(Some((epoch, total))),
        _ => Err("Conversion script reported invalid training epoch counts".into()),
    }
}

pub(super) async fn run(
    command: &mut Command,
    name: &str,
    conversion_id: &str,
    progress: &ConversionProgressTracker,
    cancel: &CancellationToken,
) -> Result<()> {
    let mut script_failure = None;
    let outcome = native_process::run(command, name, cancel, |stream, line| {
        if stream == OutputStream::Stdout {
            if let Ok(observation) = serde_json::from_str::<Observation>(line) {
                let record = &observation.progress;
                if record.stage == "error" {
                    script_failure.get_or_insert_with(|| {
                        record
                            .message
                            .clone()
                            .unwrap_or_else(|| "Conversion script failed".into())
                    });
                }
                progress.update_from_script(conversion_id, record);
                if record.stage == "training" {
                    match epoch_pair(&observation) {
                        Ok(Some((epoch, total))) => {
                            progress.update_training_progress(conversion_id, epoch, total);
                        }
                        Ok(None) => {}
                        Err(message) => {
                            script_failure.get_or_insert(message);
                        }
                    }
                }
                return;
            }
        }
        debug!("[{}] {:?}: {}", conversion_id, stream, line);
    })
    .await;
    if let Err(error) = outcome {
        if matches!(error, PumasError::ConversionCancelled) {
            return Err(error);
        }
        if let Some(message) = script_failure {
            return Err(PumasError::ConversionFailed {
                message: format!("{error}; script reported: {message}"),
            });
        }
        return Err(error);
    }
    if let Some(message) = script_failure {
        return Err(PumasError::ConversionFailed { message });
    }
    Ok(())
}

#[cfg(all(test, unix))]
mod tests {
    use super::super::types::ConversionStatus;
    use super::*;
    use std::time::Duration;

    fn tracker() -> ConversionProgressTracker {
        let tracker = ConversionProgressTracker::new();
        tracker.insert(
            serde_json::from_value(serde_json::json!({
                "conversionId": "fixture", "sourceModelId": "source",
                "direction": "gguf_to_safetensors", "status": "setting_up"
            }))
            .unwrap(),
        );
        tracker
    }

    async fn execute(
        tracker: &ConversionProgressTracker,
        stdout: &str,
        stderr: &str,
        code: &str,
    ) -> Result<()> {
        let mut command = Command::new("sh");
        command.args([
            "-c",
            "printf '%s\\n' \"$1\"; printf '%s\\n' \"$2\" >&2; exit \"$3\"",
            "fixture",
            stdout,
            stderr,
            code,
        ]);
        run(
            &mut command,
            "fixture script",
            "fixture",
            tracker,
            &CancellationToken::new(),
        )
        .await
    }

    #[tokio::test]
    async fn projects_script_phases_but_not_stderr_or_terminal_receipts() {
        let tracker = tracker();
        for (stage, expected) in [
            ("setup", ConversionStatus::SettingUp),
            ("loading", ConversionStatus::SettingUp),
            ("calibrating", ConversionStatus::Calibrating),
            ("quantizing", ConversionStatus::Quantizing),
            ("training", ConversionStatus::Training),
            ("writing", ConversionStatus::Writing),
            ("exporting", ConversionStatus::Writing),
            ("complete", ConversionStatus::Writing),
        ] {
            execute(
                &tracker,
                &format!("fixture diagnostic\n{{\"stage\":\"{stage}\"}}"),
                r#"{"stage":"error","message":"stderr is diagnostic"}"#,
                "0",
            )
            .await
            .unwrap();
            let observed = tracker.get("fixture").unwrap();
            assert_eq!(observed.status, expected);
            assert_eq!(observed.error, None);
        }
        assert_eq!(tracker.get("fixture").unwrap().progress, Some(0.95));
    }

    #[tokio::test]
    async fn epoch_counts_measure_started_epochs_and_invalid_pairs_fail() {
        for (fields, expected) in [
            (r#", "epoch":1,"epochs_total":4"#, Some(0.0)),
            (r#", "epoch":4,"epochs_total":4"#, Some(0.75)),
            (r#", "epoch":2"#, None),
            (r#", "epochs_total":4"#, None),
            ("", None),
        ] {
            let tracker = tracker();
            execute(
                &tracker,
                &format!(r#"{{"stage":"training"{fields}}}"#),
                "",
                "0",
            )
            .await
            .unwrap();
            let observed = tracker.get("fixture").unwrap();
            assert_eq!(observed.status, ConversionStatus::Training);
            assert_eq!(observed.progress, expected);
        }
        for (epoch, total) in [
            ("0", "4"),
            ("5", "4"),
            ("1", "0"),
            ("-1", "4"),
            ("1.5", "4"),
            ("4294967296", "4294967296"),
        ] {
            let tracker = tracker();
            let result = execute(
                &tracker,
                &format!(r#"{{"stage":"training","epoch":{epoch},"epochs_total":{total}}}"#),
                "",
                "0",
            )
            .await;
            assert!(
                matches!(result, Err(PumasError::ConversionFailed { message }) if message.contains("invalid training epoch"))
            );
            assert_eq!(
                tracker.get("fixture").unwrap().status,
                ConversionStatus::Training
            );
            assert_eq!(tracker.get("fixture").unwrap().error, None);
        }
    }

    #[tokio::test]
    async fn retains_first_script_failure_on_successful_and_failed_exit() {
        for code in ["0", "7"] {
            let tracker = tracker();
            let result = execute(&tracker, "{\"stage\":\"error\",\"message\":\"first failure\"}\n{\"stage\":\"error\",\"message\":\"second failure\"}\n{\"stage\":\"complete\"}", "", code).await;
            let Err(PumasError::ConversionFailed { message }) = result else {
                panic!("expected retained script failure");
            };
            assert!(message.contains("first failure"));
            assert!(!message.contains("second failure"));
            assert_eq!(
                message.contains("subprocess exited unsuccessfully"),
                code == "7"
            );
            assert_eq!(
                tracker.get("fixture").unwrap().status,
                ConversionStatus::Writing
            );
            assert_eq!(tracker.get("fixture").unwrap().error, None);
        }
    }

    #[tokio::test]
    async fn cancellation_takes_precedence_over_script_error_after_cleanup() {
        let tracker = tracker();
        let cancel = CancellationToken::new();
        let mut command = Command::new("sh");
        command.args(["-c", "printf '%s\\n' '{\"stage\":\"error\",\"message\":\"failure\"}' '{\"stage\":\"complete\",\"output_size\":17}'; exec sleep 10"]);
        let observer = async {
            let observed = tokio::time::timeout(Duration::from_secs(5), async {
                while tracker.get("fixture").unwrap().estimated_output_size != Some(17) {
                    tokio::time::sleep(Duration::from_millis(5)).await;
                }
            })
            .await;
            cancel.cancel();
            observed
        };
        let (result, observed) = tokio::join!(
            run(&mut command, "fixture script", "fixture", &tracker, &cancel),
            observer
        );
        observed.expect("script record observed before cancellation");
        assert!(matches!(result, Err(PumasError::ConversionCancelled)));
        assert_eq!(
            tracker.get("fixture").unwrap().status,
            ConversionStatus::Writing
        );
        assert_eq!(tracker.get("fixture").unwrap().error, None);
    }
}
