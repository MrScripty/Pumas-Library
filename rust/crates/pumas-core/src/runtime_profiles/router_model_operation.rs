//! Exclusive model mutations within an immutable owned router launch generation.
//!
//! The noncloneable guard and its pending-command flag prevent Idle while a
//! command is queued/running or its completion is unobserved. Cancellation
//! makes the generation Uncertain; only a fresh owned launch resets it. The
//! retained process worker performs disk IO without mutexes and drains before
//! Stop joins, so writes cannot escape into a replacement generation.

use super::process_owner::{OwnedRuntimeProfileObservation, RuntimeProfileProcessOwner};
use super::{
    RuntimeProfileBinaryLaunchKind, RuntimeProfileLaunchSpec, RuntimeProfileLaunchStrategy,
};
use crate::models::RuntimeProfileId;
use crate::{PumasError, Result};
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

fn failure(message: impl Into<String>) -> PumasError {
    PumasError::Other(message.into())
}

#[derive(Debug, PartialEq, Eq)]
enum ModelOperationState {
    Idle,
    Busy,
    Uncertain,
}

#[derive(Debug)]
pub(super) struct RouterModelState {
    path: PathBuf,
    expected: Option<Vec<u8>>,
    pending: Option<ModelContextCommand>,
    #[cfg(test)]
    command_gate: Option<std::sync::mpsc::Receiver<()>>,
    operation: ModelOperationState,
}

impl RouterModelState {
    pub(super) fn observation(models: &Mutex<Self>) -> Result<(Vec<u8>, bool, bool)> {
        let state = models
            .lock()
            .map_err(|_| failure("Router model state poisoned"))?;
        Ok((
            state
                .expected
                .clone()
                .ok_or_else(|| failure("Router preset was not captured at launch"))?,
            state.operation == ModelOperationState::Busy,
            state.operation == ModelOperationState::Uncertain,
        ))
    }
    #[cfg(target_os = "linux")]
    pub(super) fn for_spec(spec: &RuntimeProfileLaunchSpec) -> Option<Arc<Mutex<Self>>> {
        if spec.launch_strategy
            != RuntimeProfileLaunchStrategy::BinaryProcess(
                RuntimeProfileBinaryLaunchKind::LlamaCppRouter,
            )
        {
            return None;
        }
        Some(Arc::new(Mutex::new(Self {
            path: spec.runtime_dir.join("models-preset.ini"),
            expected: None,
            pending: None,
            #[cfg(test)]
            command_gate: None,
            operation: ModelOperationState::Idle,
        })))
    }

    #[cfg(target_os = "linux")]
    pub(super) fn capture(models: &Mutex<Self>) -> Result<()> {
        let path = models
            .lock()
            .map_err(|_| failure("Router model state poisoned"))?
            .path
            .clone();
        let bytes = read_regular_preset(&path)?;
        models
            .lock()
            .map_err(|_| failure("Router model state poisoned"))?
            .expected = Some(bytes);
        Ok(())
    }

    #[cfg(all(test, target_os = "linux"))]
    pub(super) fn set_command_gate(models: &Mutex<Self>, gate: std::sync::mpsc::Receiver<()>) {
        models.lock().unwrap().command_gate = Some(gate);
    }

    #[cfg(target_os = "linux")]
    pub(super) fn reject_pending(models: &Mutex<Self>) {
        let pending = models
            .lock()
            .ok()
            .and_then(|mut state| state.pending.take());
        if let Some(command) = pending {
            let _ = command
                .reply
                .send(Err(failure("Owned router worker is stopping")));
        }
    }

    #[cfg(target_os = "linux")]
    pub(super) fn process_pending(
        models: &Mutex<Self>,
        current: impl Fn(&OwnedRuntimeProfileObservation) -> Result<()>,
    ) -> Result<()> {
        let pending = models
            .lock()
            .map_err(|_| failure("Router model state poisoned"))?
            .pending
            .take();
        if let Some(command) = pending {
            #[cfg(test)]
            {
                let gate = models
                    .lock()
                    .map_err(|_| failure("Router model state poisoned"))?
                    .command_gate
                    .take();
                if let Some(gate) = gate {
                    let _ = gate.recv();
                }
            }
            let result = command.execute(models, current);
            // The retained worker observes writes even if their waiter was cancelled.
            let _ = command.reply.send(result);
        }
        Ok(())
    }
}

#[derive(Debug)]
struct ModelContextCommand {
    receipt: OwnedRuntimeProfileObservation,
    expected: Vec<u8>,
    replacement: Vec<u8>,
    reply: tokio::sync::oneshot::Sender<Result<bool>>,
}

impl ModelContextCommand {
    #[cfg(target_os = "linux")]
    fn execute(
        &self,
        models: &Mutex<RouterModelState>,
        current: impl Fn(&OwnedRuntimeProfileObservation) -> Result<()>,
    ) -> Result<bool> {
        let check = || -> Result<PathBuf> {
            current(&self.receipt)?;
            let state = models
                .lock()
                .map_err(|_| failure("Router model state poisoned"))?;
            if state.operation != ModelOperationState::Busy || self.reply.is_closed() {
                return Err(failure(
                    "Router model operation was cancelled or became uncertain",
                ));
            }
            if state.expected.as_ref() != Some(&self.expected) {
                return Err(failure("Router preset version is no longer current"));
            }
            Ok(state.path.clone())
        };
        let path = check()?;
        let metadata =
            std::fs::symlink_metadata(&path).map_err(|e| PumasError::io_with_path(e, &path))?;
        if read_regular_preset(&path)? != self.expected {
            return Err(failure("Owned router preset was externally edited"));
        }
        if self.replacement == self.expected {
            return Ok(false);
        }
        let parent = path
            .parent()
            .ok_or_else(|| failure("Router preset directory is absent"))?;
        let mut temporary = tempfile::NamedTempFile::new_in(parent)
            .map_err(|e| PumasError::io_with_path(e, parent))?;
        temporary
            .write_all(&self.replacement)
            .map_err(|e| PumasError::io_with_path(e, &path))?;
        temporary
            .flush()
            .map_err(|e| PumasError::io_with_path(e, &path))?;
        check()?;
        let latest =
            std::fs::symlink_metadata(&path).map_err(|e| PumasError::io_with_path(e, &path))?;
        if !same_file_version(&metadata, &latest) {
            return Err(failure("Owned router preset changed during preparation"));
        }
        // This worker retains generation custody through IO and cleanup. Stop
        // cannot join it or admit a successor while this commit is in progress.
        temporary
            .persist(&path)
            .map_err(|e| PumasError::io_with_path(e.error, &path))?;
        models
            .lock()
            .map_err(|_| failure("Router model state poisoned"))?
            .expected = Some(self.replacement.clone());
        Ok(true)
    }
}

#[cfg(target_os = "linux")]
fn same_file_version(before: &std::fs::Metadata, after: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    after.file_type().is_file()
        && before.dev() == after.dev()
        && before.ino() == after.ino()
        && before.len() == after.len()
        && before.mtime() == after.mtime()
        && before.mtime_nsec() == after.mtime_nsec()
        && before.ctime() == after.ctime()
        && before.ctime_nsec() == after.ctime_nsec()
}

/// Exclusive model-operation custody for one owned router generation.
///
/// Call `mark_mutating` before the first runtime HTTP mutation. Context edits
/// arm automatically. Only confirmed completion may call `finish`; dropping an
/// armed guard leaves the session uncertain until explicit stop/restart.
#[must_use = "finish confirmed operations; dropping an armed operation leaves the session uncertain"]
pub struct OwnedRouterModelOperation {
    owner: Arc<RuntimeProfileProcessOwner>,
    profile_id: RuntimeProfileId,
    receipt: OwnedRuntimeProfileObservation,
    models: Arc<Mutex<RouterModelState>>,
    armed: bool,
    command_pending: bool,
    finished: bool,
}

impl OwnedRouterModelOperation {
    pub(super) fn begin(
        owner: Arc<RuntimeProfileProcessOwner>,
        profile_id: RuntimeProfileId,
        receipt: OwnedRuntimeProfileObservation,
        models: Arc<Mutex<RouterModelState>>,
    ) -> Result<Self> {
        {
            let mut state = models
                .lock()
                .map_err(|_| failure("Router model state poisoned"))?;
            match state.operation {
                ModelOperationState::Idle => state.operation = ModelOperationState::Busy,
                ModelOperationState::Busy => return Err(failure("Router model operation is busy")),
                ModelOperationState::Uncertain => {
                    return Err(failure(
                        "Router model state is uncertain; explicitly stop and restart the profile",
                    ))
                }
            }
        }
        Ok(Self {
            owner,
            profile_id,
            receipt,
            models,
            armed: false,
            command_pending: false,
            finished: false,
        })
    }

    /// Recheck the exact child/listener and arm before the first external mutation.
    pub fn mark_mutating(&mut self) -> Result<()> {
        self.owner
            .with_running_session(&self.profile_id, &self.receipt, || {
                self.armed = true;
            })
    }

    /// Replace only the selected model's context in the retained owned preset.
    ///
    /// Returns false when its context already matches. Rejects ambiguous model
    /// sections, externally edited files, and stale owner/listener receipts.
    /// The caller owns the runtime's all-models-unloaded/reload precondition.
    pub async fn set_model_context(&mut self, model_id: &str, context_size: u32) -> Result<bool> {
        if self.command_pending {
            return Err(failure("Router preset command completion is unobserved"));
        }
        let expected = self
            .models
            .lock()
            .map_err(|_| failure("Router model state poisoned"))?
            .expected
            .clone()
            .ok_or_else(|| failure("Router preset was not captured at launch"))?;
        let replacement = edit_model_context(&expected, model_id, context_size)?;
        self.replace_preset(expected, replacement).await
    }

    pub(crate) fn preset(&self) -> Result<Vec<u8>> {
        self.models
            .lock()
            .map_err(|_| failure("Router model state poisoned"))?
            .expected
            .clone()
            .ok_or_else(|| failure("Router preset was not captured at launch"))
    }

    pub(crate) async fn replace_preset(
        &mut self,
        expected: Vec<u8>,
        replacement: Vec<u8>,
    ) -> Result<bool> {
        if self.command_pending {
            return Err(failure("Router preset command completion is unobserved"));
        }
        self.mark_mutating()?;
        let (reply, receiver) = tokio::sync::oneshot::channel();
        self.command_pending = true;
        self.owner
            .with_running_session(&self.profile_id, &self.receipt, || {
                let mut state = self
                    .models
                    .lock()
                    .map_err(|_| failure("Router model state poisoned"))?;
                if state.pending.is_some() {
                    return Err(failure("Router preset command is already pending"));
                }
                state.pending = Some(ModelContextCommand {
                    receipt: self.receipt.clone(),
                    expected,
                    replacement,
                    reply,
                });
                Ok(())
            })??;
        let result = receiver
            .await
            .map_err(|_| failure("Owned router preset worker stopped before completion"))?;
        self.command_pending = false;
        result
    }

    /// Release custody only after confirmed completion and a current owner check.
    pub fn finish(mut self) -> Result<()> {
        if self.command_pending {
            return Err(failure("Router preset command completion is unobserved"));
        }
        self.owner
            .with_running_session(&self.profile_id, &self.receipt, || {
                let mut state = self
                    .models
                    .lock()
                    .map_err(|_| failure("Router model state poisoned"))?;
                state.operation = ModelOperationState::Idle;
                self.finished = true;
                Ok(())
            })?
    }
}

impl Drop for OwnedRouterModelOperation {
    fn drop(&mut self) {
        if !self.finished {
            if let Ok(mut state) = self.models.lock() {
                state.operation = if self.armed {
                    ModelOperationState::Uncertain
                } else {
                    ModelOperationState::Idle
                };
            }
        }
    }
}

fn read_regular_preset(path: &std::path::Path) -> Result<Vec<u8>> {
    let metadata =
        std::fs::symlink_metadata(path).map_err(|e| PumasError::io_with_path(e, path))?;
    if !metadata.file_type().is_file() {
        return Err(failure("Owned router preset must be a regular file"));
    }
    std::fs::read(path).map_err(|e| PumasError::io_with_path(e, path))
}

fn edit_model_context(bytes: &[u8], model_id: &str, context_size: u32) -> Result<Vec<u8>> {
    if context_size == 0
        || model_id.is_empty()
        || model_id == "*"
        || model_id.trim() != model_id
        || model_id.contains(['[', ']', '\r', '\n'])
    {
        return Err(failure("Invalid or ambiguous router model context target"));
    }
    let text = std::str::from_utf8(bytes).map_err(|_| failure("Router preset is not UTF-8"))?;
    let section = format!("[{model_id}]");
    let mut selected = None;
    let mut offset = 0;
    let mut end = bytes.len();
    for line in text.split_inclusive('\n') {
        let content = line.trim();
        if content.starts_with('[') && content.ends_with(']') {
            if selected.is_some() && end == bytes.len() {
                end = offset;
            }
            if content == section {
                if selected.is_some() {
                    return Err(failure("Duplicate router model section"));
                }
                selected = Some(offset + line.len());
                end = bytes.len();
            }
        }
        offset += line.len();
    }
    let start = selected.ok_or_else(|| failure("Selected router model section is absent"))?;
    let mut context = None;
    offset = start;
    for line in text[start..end].split_inclusive('\n') {
        let body = line.trim_end_matches(['\r', '\n']);
        if let Some((key, value)) = body.split_once('=') {
            if key.trim() == "ctx-size" {
                if context.is_some() {
                    return Err(failure("Duplicate router model ctx-size key"));
                }
                let leading = value.len() - value.trim_start().len();
                let value_start = offset + key.len() + 1 + leading;
                let trimmed = value.trim();
                let token = trimmed.split_whitespace().next().unwrap_or("");
                if !token.is_empty() && token.parse::<u32>().is_err() {
                    return Err(failure("Invalid router model ctx-size value"));
                }
                let suffix = trimmed[token.len()..].trim_start();
                if !suffix.is_empty() && !suffix.starts_with(['#', ';']) {
                    return Err(failure("Ambiguous router model ctx-size value"));
                }
                let value_end = value_start + token.len();
                context = Some(value_start..value_end);
            }
        }
        offset += line.len();
    }
    let mut output = text.to_string();
    if let Some(range) = context {
        output.replace_range(range, &context_size.to_string());
    } else {
        let newline = if text[..start].ends_with("\r\n") {
            "\r\n"
        } else {
            "\n"
        };
        let prefix = if text[..start].ends_with('\n') {
            ""
        } else {
            newline
        };
        output.insert_str(
            start,
            &format!("{prefix}ctx-size = {context_size}{newline}"),
        );
    }
    Ok(output.into_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_edit_preserves_other_stanzas_and_bytes() {
        let original = b"version = 1\r\n[*]\r\nctx-size = 4096\r\n[model/a]\r\nmodel = /a\r\nctx-size = 8192  \r\n\r\n[model/b]\r\nctx-size = 1024\r\n";
        let expected = String::from_utf8(original.to_vec())
            .unwrap()
            .replace("8192", "18000");
        assert_eq!(
            edit_model_context(original, "model/a", 18000).unwrap(),
            expected.as_bytes()
        );
        let inserted =
            edit_model_context(b"[*]\nx = y\n[a]\nmodel = a\n[b]\nmodel = b", "a", 18000).unwrap();
        assert_eq!(
            inserted,
            b"[*]\nx = y\n[a]\nctx-size = 18000\nmodel = a\n[b]\nmodel = b"
        );
        assert_eq!(edit_model_context(&inserted, "a", 18000).unwrap(), inserted);
    }

    #[test]
    fn context_edit_rejects_absence_duplicates_and_sanitization_ambiguity() {
        for (preset, target) in [
            ("[a]\nx=y\n", "missing"),
            ("[a]\nx=y\n[a]\nx=z\n", "a"),
            ("[a]\nctx-size=1\nctx-size=2\n", "a"),
            ("[a_b]\nx=y\n", "a[b"),
            ("[*]\nx=y", "*"),
        ] {
            assert!(edit_model_context(preset.as_bytes(), target, 18000).is_err());
        }
        assert!(edit_model_context(b"[a]\n", "a", 0).is_err());
    }
    #[test]
    fn context_edit_preserves_inline_comments_and_empty_values() {
        assert_eq!(
            edit_model_context(b"[a]\nctx-size = 8192  # retained\n", "a", 18000).unwrap(),
            b"[a]\nctx-size = 18000  # retained\n"
        );
        assert_eq!(
            edit_model_context(b"[a]\nctx-size =   \n", "a", 18000).unwrap(),
            b"[a]\nctx-size =   18000\n"
        );
        assert!(edit_model_context(b"[a]\nctx-size = unknown\n", "a", 18000).is_err());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn preset_read_rejects_symlink_and_fingerprint_detects_replacement() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("models-preset.ini");
        std::fs::write(&path, b"[a]\n").unwrap();
        let before = std::fs::symlink_metadata(&path).unwrap();
        let link = root.path().join("linked.ini");
        std::os::unix::fs::symlink(&path, &link).unwrap();
        assert!(read_regular_preset(&link).is_err());
        let replacement = root.path().join("replacement.ini");
        std::fs::write(&replacement, b"[a]\n").unwrap();
        std::fs::rename(replacement, &path).unwrap();
        assert!(!same_file_version(
            &before,
            &std::fs::symlink_metadata(&path).unwrap()
        ));
    }
}
