//! Read-only import probes. Async callers share a retained worker, not a cached
//! readiness answer. Synchronous callers own their blocking invocation and must
//! join it before shutting down; dropping this owner only signals async work.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::sync::watch;
use tokio::task::JoinHandle;

use super::setup::Failure;
use crate::cancel::CancellationToken;
use crate::{PumasError, Result};

type Outcome = std::result::Result<bool, Failure>;
const PROBE_TIMEOUT: Duration = Duration::from_secs(5);
pub(super) const BASE_CONVERSION_IMPORTS: &str = "import numpy, sentencepiece; from gguf import GGUFReader, GGUFWriter; from safetensors import safe_open; from safetensors.numpy import save_file";

pub(super) fn usable_artifact(metadata: &std::fs::Metadata, executable: bool) -> bool {
    if !metadata.is_file() || metadata.len() == 0 {
        return false;
    }
    #[cfg(unix)]
    if executable {
        use std::os::unix::fs::PermissionsExt;
        return metadata.permissions().mode() & 0o111 != 0;
    }
    #[cfg(not(unix))]
    let _ = executable;
    true
}

fn public(error: Failure) -> PumasError {
    match error {
        Failure::Cancelled => PumasError::ConversionCancelled,
        Failure::Failed(message) | Failure::CommandNotFound(message) => {
            PumasError::ConversionFailed { message }
        }
    }
}

#[derive(Clone)]
struct Specification {
    python: PathBuf,
    name: &'static str,
    imports: &'static str,
    artifacts: Vec<(PathBuf, bool)>,
}

impl Specification {
    fn execute(&self, cancel: &CancellationToken) -> Outcome {
        super::setup::check_cancel(cancel)?;
        for (path, executable) in &self.artifacts {
            let metadata = match std::fs::metadata(path) {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
                Err(error) => {
                    return Err(super::setup::failed(
                        &format!(
                            "Inspecting {} readiness artifact {}",
                            self.name,
                            path.display()
                        ),
                        error,
                    ))
                }
            };
            if !usable_artifact(&metadata, *executable) {
                return Ok(false);
            }
        }
        super::backend_setup::imports_ready(
            &self.python,
            self.name,
            self.imports,
            cancel,
            PROBE_TIMEOUT,
        )
    }
}

struct Receipt {
    handle: Option<JoinHandle<()>>,
    outcome: Option<Outcome>,
}

struct Operation {
    cancel: CancellationToken,
    completion: watch::Receiver<Option<Outcome>>,
    receipt: tokio::sync::Mutex<Receipt>,
}

impl Operation {
    fn finished(&self) -> bool {
        self.completion.borrow().is_some() || self.completion.has_changed().is_err()
    }

    async fn observe(&self) -> Outcome {
        let mut completion = self.completion.clone();
        let outcome = loop {
            if let Some(outcome) = completion.borrow().clone() {
                break outcome;
            }
            if completion.changed().await.is_err() {
                break completion.borrow().clone().unwrap_or_else(|| {
                    Err(Failure::Failed(
                        "Readiness probe completion was lost".into(),
                    ))
                });
            }
        };
        let mut receipt = self.receipt.lock().await;
        if let Some(handle) = receipt.handle.as_mut() {
            let observed = if handle.await.is_err() {
                Err(Failure::Failed("Readiness probe worker failed".into()))
            } else {
                outcome
            };
            receipt.outcome = Some(observed);
            receipt.handle = None;
        }
        receipt.outcome.clone().expect("readiness receipt observed")
    }
}

#[derive(Default)]
struct State {
    closed: bool,
    operation: Option<Arc<Operation>>,
}

pub(super) struct ProbeOwner {
    spec: Specification,
    state: Mutex<State>,
}

impl ProbeOwner {
    pub(super) fn new(
        python: PathBuf,
        name: &'static str,
        imports: &'static str,
        artifacts: Vec<(PathBuf, bool)>,
    ) -> Self {
        Self {
            spec: Specification {
                python,
                name,
                imports,
                artifacts,
            },
            state: Mutex::new(State::default()),
        }
    }

    fn start(&self) -> Arc<Operation> {
        let spec = self.spec.clone();
        let cancel = CancellationToken::new();
        let worker_cancel = cancel.clone();
        let (sender, completion) = watch::channel(None);
        let handle = tokio::task::spawn_blocking(move || {
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                spec.execute(&worker_cancel)
            }))
            .unwrap_or_else(|_| Err(Failure::Failed("Readiness probe worker panicked".into())));
            sender.send_replace(Some(outcome));
        });
        Arc::new(Operation {
            cancel,
            completion,
            receipt: tokio::sync::Mutex::new(Receipt {
                handle: Some(handle),
                outcome: None,
            }),
        })
    }

    pub(super) async fn check(&self) -> Result<bool> {
        let (operation, previous) = {
            let mut state = self.state.lock().expect("readiness owner poisoned");
            if state.closed {
                return Err(public(Failure::Cancelled));
            }
            match state.operation.as_ref() {
                Some(operation) => (operation.clone(), operation.finished()),
                None => {
                    let operation = self.start();
                    state.operation = Some(operation.clone());
                    (operation, false)
                }
            }
        };
        let result = operation.observe().await;
        if !previous {
            return result.map_err(public);
        }
        let next = {
            let mut state = self.state.lock().expect("readiness owner poisoned");
            if state.closed {
                return Err(public(Failure::Cancelled));
            }
            if state
                .operation
                .as_ref()
                .is_some_and(|current| Arc::ptr_eq(current, &operation))
            {
                state.operation = Some(self.start());
            }
            state
                .operation
                .clone()
                .expect("retained readiness operation")
        };
        next.observe().await.map_err(public)
    }

    /// Caller-owned blocking work: no runtime or background worker is created.
    pub(super) fn check_blocking(&self) -> Result<bool> {
        if self.state.lock().expect("readiness owner poisoned").closed {
            return Err(public(Failure::Cancelled));
        }
        self.spec.execute(&CancellationToken::new()).map_err(public)
    }

    pub(super) fn close(&self) {
        let mut state = self.state.lock().expect("readiness owner poisoned");
        state.closed = true;
        if let Some(operation) = &state.operation {
            operation.cancel.cancel();
        }
    }

    pub(super) async fn shutdown(&self) -> Result<()> {
        self.close();
        let operation = self
            .state
            .lock()
            .expect("readiness owner poisoned")
            .operation
            .clone();
        match operation {
            Some(operation) => match operation.observe().await {
                Ok(_) | Err(Failure::Cancelled) => Ok(()),
                Err(error) => Err(public(error)),
            },
            None => Ok(()),
        }
    }
}

impl Drop for ProbeOwner {
    fn drop(&mut self) {
        if let Some(operation) = &self
            .state
            .get_mut()
            .expect("readiness owner poisoned")
            .operation
        {
            operation.cancel.cancel();
        }
    }
}

pub(super) async fn shutdown_backend(
    setup: &super::setup::SetupOwner,
    probes: &ProbeOwner,
) -> Result<()> {
    setup.close();
    probes.close();
    let setup_result = setup.shutdown().await;
    let probe_result = probes.shutdown().await;
    match (setup_result, probe_result) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
        (Err(setup), Err(probe)) => Err(PumasError::ConversionFailed {
            message: format!("{setup}; {probe}"),
        }),
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::super::progress::ConversionProgressTracker;
    use super::super::{QuantBackend, QuantOption, QuantizationBackend, QuantizeParams};
    use super::*;

    struct DownstreamBackend;

    #[async_trait::async_trait]
    impl QuantizationBackend for DownstreamBackend {
        fn name(&self) -> &str {
            "downstream fixture"
        }
        fn backend_id(&self) -> QuantBackend {
            QuantBackend::LlamaCpp
        }
        fn is_ready(&self) -> bool {
            panic!("blocking readiness must not be called by async default")
        }
        async fn ensure_environment(&self) -> Result<()> {
            panic!("fixture does not install")
        }
        fn supported_quant_types(&self) -> Vec<QuantOption> {
            Vec::new()
        }
        async fn quantize(
            &self,
            _: &QuantizeParams,
            _: &ConversionProgressTracker,
            _: &CancellationToken,
        ) -> Result<PathBuf> {
            panic!("fixture does not execute conversions")
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn downstream_async_default_is_unavailable_without_invoking_sync_readiness() {
        let backend: &dyn QuantizationBackend = &DownstreamBackend;
        assert!(
            matches!(backend.is_ready_async().await, Err(PumasError::ConversionFailed { message }) if message == "Async readiness is unavailable for backend downstream fixture")
        );
    }

    fn script(path: &std::path::Path, source: &str) {
        let result = std::process::Command::new("/bin/sh")
            .args([
                "-c",
                "printf '%s' \"$2\" > \"$1\" && chmod 700 \"$1\"",
                "fixture",
            ])
            .arg(path)
            .arg(source)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
    fn owner(path: PathBuf) -> ProbeOwner {
        ProbeOwner::new(
            path.clone(),
            "fixture",
            "import fixture",
            vec![(path, true)],
        )
    }
    async fn started(path: &std::path::Path) {
        tokio::time::timeout(Duration::from_secs(3), async {
            while !path.exists() {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .expect("probe started without blocking runtime");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn async_calls_share_retained_work_and_later_reads_are_fresh() {
        let root = tempfile::tempdir().unwrap();
        let python = root.path().join("python");
        script(&python, "#!/bin/sh\nprintf 'start\\n' >> \"$0.starts\"\necho $$ > \"$0.pid\"\nattempt=0\nwhile ! test -f \"$0.release\"; do\n attempt=$((attempt+1)); if test $attempt -ge 300; then exit 7; fi\n sleep 0.01\ndone\ntest -f \"$0.ready\"\n");
        let owner = owner(python);
        let mut first = Box::pin(owner.check());
        assert!(futures::poll!(first.as_mut()).is_pending());
        started(&root.path().join("python.pid")).await;
        let mut second = Box::pin(owner.check());
        assert!(futures::poll!(second.as_mut()).is_pending());
        drop(first);
        std::fs::write(root.path().join("python.release"), "").unwrap();
        assert!(!second.await.unwrap());
        assert_eq!(
            std::fs::read_to_string(root.path().join("python.starts")).unwrap(),
            "start\n"
        );
        std::fs::write(root.path().join("python.ready"), "").unwrap();
        assert!(owner.check().await.unwrap());
        assert_eq!(
            std::fs::read_to_string(root.path().join("python.starts")).unwrap(),
            "start\nstart\n"
        );
        owner.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn interrupted_shutdown_retains_cleanup_and_closes_admission() {
        let root = tempfile::tempdir().unwrap();
        let python = root.path().join("python");
        script(&python, "#!/bin/sh\necho $$ > \"$0.pid\"\nexec sleep 10\n");
        let owner = owner(python);
        let mut pending = Box::pin(owner.check());
        assert!(futures::poll!(pending.as_mut()).is_pending());
        let pid_file = root.path().join("python.pid");
        started(&pid_file).await;
        let pid = std::fs::read_to_string(pid_file).unwrap();
        let mut shutdown = Box::pin(owner.shutdown());
        let _ = futures::poll!(shutdown.as_mut());
        drop(shutdown);
        drop(pending);
        assert!(matches!(
            owner.check().await,
            Err(PumasError::ConversionCancelled)
        ));
        owner.shutdown().await.unwrap();
        owner.shutdown().await.unwrap();
        assert!(!std::path::Path::new(&format!("/proc/{}", pid.trim())).exists());
    }

    #[tokio::test]
    async fn missing_and_normal_exit_are_false_but_signal_failure_is_retained() {
        let root = tempfile::tempdir().unwrap();
        let missing = owner(root.path().join("missing"));
        assert!(!missing.check().await.unwrap());
        missing.shutdown().await.unwrap();
        for source in ["#!/bin/sh\nexit 1\n", "#!/bin/sh\nkill -TERM $$\n"] {
            let python = root.path().join("python");
            script(&python, source);
            let owner = owner(python);
            if source.contains("exit 1") {
                assert!(!owner.check().await.unwrap());
                owner.shutdown().await.unwrap();
            } else {
                let first = owner.check().await.unwrap_err().to_string();
                assert_eq!(owner.shutdown().await.unwrap_err().to_string(), first);
                assert_eq!(owner.shutdown().await.unwrap_err().to_string(), first);
            }
        }
    }

    #[test]
    fn synchronous_checks_need_no_runtime_and_preserve_artifact_errors() {
        use std::os::unix::fs::PermissionsExt;
        let root = tempfile::tempdir().unwrap();
        let python = root.path().join("python");
        script(&python, "#!/bin/sh\nexit 0\n");
        let owner = owner(python.clone());
        assert!(owner.check_blocking().unwrap());
        std::fs::set_permissions(&python, std::fs::Permissions::from_mode(0o600)).unwrap();
        assert!(!owner.check_blocking().unwrap());
        std::fs::remove_file(&python).unwrap();
        std::os::unix::fs::symlink(&python, &python).unwrap();
        assert!(
            owner.check_blocking().is_err(),
            "metadata failure is not missing readiness"
        );
        owner.close();
        assert!(matches!(
            owner.check_blocking(),
            Err(PumasError::ConversionCancelled)
        ));
    }

    #[tokio::test]
    async fn lost_completion_and_panicked_worker_keep_an_observed_failure() {
        let (sender, completion) = watch::channel(None);
        let handle = tokio::spawn(async move {
            drop(sender);
            panic!("controlled probe worker panic");
        });
        let operation = Operation {
            cancel: CancellationToken::new(),
            completion,
            receipt: tokio::sync::Mutex::new(Receipt {
                handle: Some(handle),
                outcome: None,
            }),
        };
        for _ in 0..2 {
            assert!(
                matches!(operation.observe().await, Err(Failure::Failed(message)) if message == "Readiness probe worker failed")
            );
        }
        assert!(operation.receipt.lock().await.handle.is_none());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn owned_probe_timeout_reaps_before_retaining_failure() {
        let root = tempfile::tempdir().unwrap();
        let python = root.path().join("python");
        script(&python, "#!/bin/sh\necho $$ > \"$0.pid\"\nexec sleep 10\n");
        let owner = owner(python);
        let before = tokio::time::Instant::now();
        let error = owner.check().await.unwrap_err().to_string();
        assert!(
            before.elapsed() >= PROBE_TIMEOUT,
            "actual probe deadline elapsed"
        );
        assert!(error.contains("Checking fixture imports"), "{error}");
        assert!(
            error.contains("Conversion setup command did not complete successfully"),
            "{error}"
        );
        let pid = std::fs::read_to_string(root.path().join("python.pid")).unwrap();
        assert!(
            !std::path::Path::new(&format!("/proc/{}", pid.trim())).exists(),
            "probe reaped before timeout receipt"
        );
        assert_eq!(owner.shutdown().await.unwrap_err().to_string(), error);
        assert_eq!(owner.shutdown().await.unwrap_err().to_string(), error);
    }
}
