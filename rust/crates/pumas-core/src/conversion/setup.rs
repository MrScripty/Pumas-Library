//! Built-in conversion/quantization setup custody. The configured launcher-data directory and its
//! lock file must not be replaced or unlinked while an owner is active. This is
//! advisory setup exclusion, not a hostile-filesystem capability. Explicit
//! shutdown must complete before the embedding application stops its runtime.

use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tokio::sync::watch;
use tokio::task::JoinHandle;

use super::{
    manager::probe_conversion_environment, scripts, ConversionSetupSnapshot, ConversionSetupStatus,
};
use crate::cancel::CancellationToken;
use crate::{PumasError, Result};

pub(super) const COMMAND_TIMEOUT: Duration = Duration::from_secs(15 * 60);
const POLL_INTERVAL: Duration = Duration::from_millis(20);

#[derive(Clone, Debug)]
pub(super) enum Failure {
    Cancelled,
    Failed(String),
    CommandNotFound(String),
}

impl Failure {
    fn public(self) -> PumasError {
        match self {
            Self::Cancelled => PumasError::InstallationCancelled,
            Self::Failed(message) | Self::CommandNotFound(message) => {
                PumasError::ConversionFailed { message }
            }
        }
    }
}

pub(super) type Outcome = std::result::Result<(), Failure>;

struct Operation {
    id: String,
    failure: Mutex<Option<Failure>>,
    cancel: CancellationToken,
    completion: watch::Receiver<Option<Outcome>>,
    // Keeping the handle inside this mutex makes an interrupted join resumable.
    worker: tokio::sync::Mutex<WorkerReceipt>,
}

struct WorkerReceipt {
    handle: Option<JoinHandle<()>>,
}

impl Operation {
    fn snapshot(&self) -> ConversionSetupSnapshot {
        let failure = self
            .failure
            .lock()
            .expect("setup failure receipt poisoned")
            .clone();
        let outcome = failure
            .map(Err)
            .or_else(|| self.completion.borrow().clone())
            .or_else(|| {
                if self.completion.has_changed().is_err() {
                    // Publication may race the first read. Re-read after closure
                    // before classifying a missing terminal receipt as failure.
                    self.completion.borrow().clone().or_else(|| {
                        Some(Err(Failure::Failed(
                            "Conversion setup completion was lost".into(),
                        )))
                    })
                } else {
                    None
                }
            });
        let (status, error) = match outcome {
            None => (ConversionSetupStatus::InProgress, None),
            Some(Ok(())) => (ConversionSetupStatus::Completed, None),
            Some(Err(Failure::Cancelled)) => (ConversionSetupStatus::Cancelled, None),
            Some(Err(Failure::Failed(reason) | Failure::CommandNotFound(reason))) => {
                (ConversionSetupStatus::Failed, Some(reason))
            }
        };
        ConversionSetupSnapshot {
            operation_id: self.id.clone(),
            status,
            error,
        }
    }

    async fn observe(&self) -> Outcome {
        let mut receiver = self.completion.clone();
        let outcome = loop {
            if let Some(outcome) = receiver.borrow().clone() {
                break outcome;
            }
            if receiver.changed().await.is_err() {
                break Err(Failure::Failed(
                    "Conversion setup completion was lost".into(),
                ));
            }
        };
        let mut worker = self.worker.lock().await;
        if let Some(task) = worker.handle.as_mut() {
            if task.await.is_err() {
                *self.failure.lock().expect("setup failure receipt poisoned") =
                    Some(Failure::Failed("Conversion setup worker failed".into()));
            }
            worker.handle = None;
        }
        match self
            .failure
            .lock()
            .expect("setup failure receipt poisoned")
            .as_ref()
        {
            Some(failure) => Err(failure.clone()),
            None => outcome,
        }
    }
}

#[derive(Default)]
struct State {
    closed: bool,
    operation: Option<Arc<Operation>>,
}

pub(super) struct SetupOwner {
    root: PathBuf,
    state: Mutex<State>,
    backend: super::QuantBackend,
    programs: super::backend_setup::Programs,
}

impl SetupOwner {
    pub(super) fn new(root: PathBuf) -> Self {
        Self::for_backend(root, super::QuantBackend::PythonConversion)
    }

    pub(super) fn for_backend(root: PathBuf, backend: super::QuantBackend) -> Self {
        Self {
            root,
            state: Mutex::new(State::default()),
            backend,
            programs: super::backend_setup::Programs::default(),
        }
    }

    #[cfg(all(test, target_os = "linux"))]
    pub(super) fn set_programs(&mut self, programs: super::backend_setup::Programs) {
        self.programs = programs;
    }

    pub(super) async fn ensure(&self) -> Result<()> {
        let (operation, previous) = {
            let mut state = self.state.lock().expect("conversion setup owner poisoned");
            if state.closed {
                return Err(PumasError::InstallationCancelled);
            }
            match state.operation.as_ref() {
                Some(operation) => (
                    Arc::clone(operation),
                    operation.snapshot().status != ConversionSetupStatus::InProgress,
                ),
                None => {
                    let operation = self.start();
                    state.operation = Some(Arc::clone(&operation));
                    (operation, false)
                }
            }
        };
        let outcome = operation.observe().await;
        if !previous {
            return outcome.map_err(Failure::public);
        }
        // An explicit new request may retry a completed operation, but only
        // after its worker has been observed. Existing waiters keep its result.
        let next = {
            let mut state = self.state.lock().expect("conversion setup owner poisoned");
            if state.closed {
                return Err(PumasError::InstallationCancelled);
            }
            if state
                .operation
                .as_ref()
                .is_some_and(|current| Arc::ptr_eq(current, &operation))
            {
                state.operation = Some(self.start());
            }
            state.operation.clone().expect("retained setup operation")
        };
        next.observe().await.map_err(Failure::public)
    }

    pub(super) fn snapshot(&self) -> Option<ConversionSetupSnapshot> {
        self.state
            .lock()
            .expect("conversion setup owner poisoned")
            .operation
            .as_ref()
            .map(|operation| operation.snapshot())
    }

    pub(super) async fn start_or_get(
        &self,
        expected_previous: Option<&str>,
    ) -> Result<ConversionSetupSnapshot> {
        if expected_previous.is_some_and(|id| {
            id.len() != 36
                || uuid::Uuid::parse_str(id).map_or(true, |parsed| parsed.to_string() != id)
        }) {
            return Err(PumasError::InvalidParams {
                message:
                    "Expected setup operation ID must be a canonical lower-case hyphenated UUID"
                        .into(),
            });
        }
        let previous = {
            let mut state = self.state.lock().expect("conversion setup owner poisoned");
            if state.closed {
                return Err(PumasError::InstallationCancelled);
            }
            match state.operation.as_ref() {
                None if expected_previous.is_some() => return Err(PumasError::InvalidParams { message: "No retained setup operation exists in this owner; refresh setup status before starting".into() }),
                None => {
                    let operation = self.start();
                    let snapshot = operation.snapshot();
                    state.operation = Some(operation);
                    return Ok(snapshot);
                }
                Some(operation) => {
                    let snapshot = operation.snapshot();
                    if expected_previous != Some(operation.id.as_str()) || snapshot.status == ConversionSetupStatus::InProgress {
                        return Ok(snapshot);
                    }
                    Arc::clone(operation)
                }
            }
        };
        // Observe the terminal worker before compare-and-swap admission. A
        // dropped retry waiter cannot erase its record or admit partial work.
        let _previous_outcome = previous.observe().await;
        let mut state = self.state.lock().expect("conversion setup owner poisoned");
        if state.closed {
            return Err(PumasError::InstallationCancelled);
        }
        if state
            .operation
            .as_ref()
            .is_some_and(|operation| Arc::ptr_eq(operation, &previous))
        {
            state.operation = Some(self.start());
        }
        Ok(state
            .operation
            .as_ref()
            .expect("retained setup operation")
            .snapshot())
    }

    fn start(&self) -> Arc<Operation> {
        let id = uuid::Uuid::new_v4().to_string();
        let root = self.root.clone();
        let backend = self.backend;
        let programs = self.programs.clone();
        let cancel = CancellationToken::new();
        let worker_cancel = cancel.clone();
        let (sender, receiver) = watch::channel(None);
        // One blocking worker per owner. It retains the lease and every child
        // independently of request cancellation and async runtime task aborts.
        let handle = tokio::task::spawn_blocking(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                execute(&root, &worker_cancel, backend, &programs)
            }))
            .unwrap_or_else(|_| Err(Failure::Failed("Conversion setup worker panicked".into())));
            sender.send_replace(Some(result));
        });
        Arc::new(Operation {
            id,
            failure: Mutex::new(None),
            cancel,
            completion: receiver,
            worker: tokio::sync::Mutex::new(WorkerReceipt {
                handle: Some(handle),
            }),
        })
    }

    pub(super) fn close(&self) {
        let operation = {
            let mut state = self.state.lock().expect("conversion setup owner poisoned");
            state.closed = true;
            state.operation.clone()
        };
        if let Some(operation) = operation {
            operation.cancel.cancel();
        }
    }

    pub(super) async fn shutdown(&self) -> Result<()> {
        let operation = {
            let mut state = self.state.lock().expect("conversion setup owner poisoned");
            state.closed = true;
            state.operation.clone()
        };
        if let Some(operation) = operation {
            operation.cancel.cancel();
            match operation.observe().await {
                Ok(()) | Err(Failure::Cancelled) => Ok(()),
                Err(error) => Err(error.public()),
            }
        } else {
            Ok(())
        }
    }
}

impl Drop for SetupOwner {
    fn drop(&mut self) {
        // Drop requests cleanup; only shutdown supplies its completion receipt.
        if let Some(operation) = self
            .state
            .get_mut()
            .expect("conversion setup owner poisoned")
            .operation
            .as_ref()
        {
            operation.cancel.cancel();
        }
    }
}

pub(super) fn check_cancel(cancel: &CancellationToken) -> Outcome {
    if cancel.is_cancelled() {
        Err(Failure::Cancelled)
    } else {
        Ok(())
    }
}

pub(super) fn failed(context: &str, error: impl std::fmt::Display) -> Failure {
    Failure::Failed(format!("{context}: {error}"))
}

struct SetupLease(Option<File>);

impl SetupLease {
    fn finish(&mut self) -> Outcome {
        let Some(_file) = self.0.as_ref() else {
            return Ok(());
        };
        #[cfg(target_os = "linux")]
        let outcome = {
            let mut first_failure = None;
            // Closing only our descriptor is insufficient when an unrelated
            // fork temporarily retains the same open-file description. Unlock
            // explicitly, but only after the setup worker drains its children.
            loop {
                match fs2::FileExt::unlock(_file) {
                    Ok(()) => break,
                    Err(error) => {
                        first_failure.get_or_insert_with(|| {
                            tracing::warn!(%error, "Retaining setup lease until unlock succeeds");
                            failed("Releasing setup lock", error)
                        });
                    }
                }
                std::thread::sleep(POLL_INTERVAL);
            }
            first_failure.map_or(Ok(()), Err)
        };
        #[cfg(not(target_os = "linux"))]
        let outcome = Ok(());
        self.0 = None;
        outcome
    }
}

impl Drop for SetupLease {
    fn drop(&mut self) {
        if let Err(error) = self.finish() {
            tracing::warn!(
                ?error,
                "Setup lease unwind release failed before eventual unlock"
            );
        }
    }
}

fn acquire(root: &Path) -> std::result::Result<(PathBuf, SetupLease), Failure> {
    let directory = root.join("launcher-data");
    std::fs::create_dir_all(&directory).map_err(|e| failed("Creating setup lock directory", e))?;
    let directory = directory
        .canonicalize()
        .map_err(|e| failed("Resolving setup directory", e))?;
    let path = directory.join("conversion-setup.lock");
    if std::fs::symlink_metadata(&path).is_ok_and(|metadata| !metadata.file_type().is_file()) {
        return Err(Failure::Failed(
            "Conversion setup lock must be a regular file".into(),
        ));
    }
    let mut options = OpenOptions::new();
    options.create(true).truncate(false).read(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW).mode(0o600);
    }
    let file = options
        .open(&path)
        .map_err(|e| failed("Opening setup lock", e))?;
    fs2::FileExt::try_lock_exclusive(&file).map_err(|error| {
        if error.kind() == std::io::ErrorKind::WouldBlock {
            Failure::Failed("Conversion environment setup is already running".into())
        } else {
            failed("Acquiring setup lock", error)
        }
    })?;
    let lease = SetupLease(Some(file));
    let root = root
        .canonicalize()
        .map_err(|e| failed("Resolving launcher root", e))?;
    Ok((root, lease))
}

fn execute(
    root: &Path,
    cancel: &CancellationToken,
    backend: super::QuantBackend,
    programs: &super::backend_setup::Programs,
) -> Outcome {
    check_cancel(cancel)?;
    let (root, mut lease) = acquire(root)?;
    let outcome = if backend == super::QuantBackend::PythonConversion {
        execute_with_lease(&root, cancel)
    } else {
        super::backend_setup::execute(&root, backend, cancel, programs)
    };
    match (outcome, lease.finish()) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
        (Err(error), Err(release_error)) => Err(Failure::Failed(format!(
            "{error:?}; setup lease release also failed: {release_error:?}"
        ))),
    }
}

fn execute_with_lease(root: &Path, cancel: &CancellationToken) -> Outcome {
    check_cancel(cancel)?;
    scripts::ensure_scripts_deployed_blocking(root)
        .map_err(|e| failed("Deploying conversion scripts", e))?;
    let python = scripts::venv_python(root);
    if probe_conversion_environment(&python, Duration::from_secs(5))
        .map_err(|e| failed("Checking conversion imports", e))?
    {
        return check_cancel(cancel);
    }
    check_cancel(cancel)?;
    if !python
        .try_exists()
        .map_err(|e| failed("Checking conversion interpreter", e))?
    {
        let mut command = Command::new("python3");
        command.arg("-m").arg("venv").arg(scripts::venv_dir(root));
        if !run_command(&mut command, cancel, COMMAND_TIMEOUT)?.success() {
            return Err(Failure::Failed(
                "Failed to create conversion virtual environment".into(),
            ));
        }
    }
    let mut upgrade = Command::new(&python);
    upgrade.args(["-m", "pip", "install", "--upgrade", "pip"]);
    if !run_command(&mut upgrade, cancel, COMMAND_TIMEOUT)?.success() {
        tracing::warn!("Conversion pip upgrade failed; continuing dependency installation");
    }
    let mut install = Command::new(&python);
    install
        .args(["-m", "pip", "install", "-r"])
        .arg(scripts::scripts_dir(root).join("requirements.txt"));
    if !run_command(&mut install, cancel, COMMAND_TIMEOUT)?.success() {
        return Err(Failure::Failed(
            "Failed to install conversion dependencies".into(),
        ));
    }
    check_cancel(cancel)?;
    if !probe_conversion_environment(&python, Duration::from_secs(5))
        .map_err(|e| failed("Checking conversion imports", e))?
    {
        return Err(Failure::Failed(
            "Conversion dependencies were installed but required imports are not ready".into(),
        ));
    }
    check_cancel(cancel)
}

pub(super) fn run_command(
    command: &mut Command,
    cancel: &CancellationToken,
    timeout: Duration,
) -> std::result::Result<ExitStatus, Failure> {
    check_cancel(cancel)?;
    #[cfg(target_os = "linux")]
    super::linux_group::ensure_supported()
        .map_err(|error| failed("Checking setup process-group support", error))?;
    // Setup has no streaming output contract. Null streams avoid unbounded pip
    // output accumulation and pipe-drain descendants retaining completion.
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let mut child = OwnedChild(Some(command.spawn().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            Failure::CommandNotFound(format!("Starting conversion setup command: {e}"))
        } else {
            failed("Starting conversion setup command", e)
        }
    })?));
    let started = Instant::now();
    loop {
        #[cfg(target_os = "linux")]
        let observed =
            super::linux_group::observe_exit(child.0.as_ref().expect("setup child owned").id());
        #[cfg(not(target_os = "linux"))]
        let observed = child.0.as_mut().expect("setup child owned").try_wait();
        if cancel.is_cancelled() || started.elapsed() >= timeout || observed.is_err() {
            child.finish()?;
            if cancel.is_cancelled() {
                return Err(Failure::Cancelled);
            }
            return Err(Failure::Failed(
                "Conversion setup command did not complete successfully".into(),
            ));
        }
        if let Some(status) = observed.map_err(|e| failed("Observing setup command", e))? {
            // A successfully exited leader must not leave background installers.
            child.finish()?;
            return Ok(status);
        }
        std::thread::sleep(POLL_INTERVAL);
    }
}

struct OwnedChild(Option<std::process::Child>);

impl OwnedChild {
    fn finish(&mut self) -> Outcome {
        if let Some(child) = self.0.as_mut() {
            let result = terminate(child);
            self.0 = None;
            result
        } else {
            Ok(())
        }
    }
}

impl Drop for OwnedChild {
    fn drop(&mut self) {
        if let Err(error) = self.finish() {
            tracing::warn!(
                ?error,
                "Conversion setup unwind cleanup failed after process observation"
            );
        }
    }
}

fn terminate(child: &mut std::process::Child) -> Outcome {
    let mut first_failure = None;
    #[cfg(target_os = "linux")]
    {
        // Linux PIDs fit in pid_t. Failure to establish or observe custody must
        // keep this worker and its outer file lease alive, never release early.
        let group = i32::try_from(child.id()).expect("Linux PID fits pid_t");
        loop {
            match super::linux_group::signal_group(child.id()) {
                Ok(()) => match super::linux_group::group_has_live_members(group) {
                    Ok(false) => break,
                    Ok(true) => {}
                    Err(error) => {
                        first_failure.get_or_insert_with(|| {
                            failed("Observing setup process-group cleanup", error)
                        });
                    }
                },
                Err(error) => {
                    first_failure
                        .get_or_insert_with(|| failed("Signalling setup process group", error));
                }
            }
            std::thread::sleep(POLL_INTERVAL);
        }
    }
    // Other targets deliberately drain the foreground command naturally. They
    // do not claim Linux process-tree cleanup: cancellation prevents later
    // steps but shutdown can wait for the installer to exit.
    loop {
        match child.wait() {
            Ok(_) => break,
            Err(error) => {
                first_failure.get_or_insert_with(|| failed("Reaping setup process", error));
            }
        }
        std::thread::sleep(POLL_INTERVAL);
    }
    first_failure.map_or(Ok(()), Err)
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::super::linux_group::group_has_live_members;
    use super::*;
    use std::future::Future;
    use std::os::unix::fs::{symlink, PermissionsExt};
    use tempfile::TempDir;

    fn fixture() -> TempDir {
        let root = tempfile::tempdir().expect("temporary setup root");
        let python = scripts::venv_python(root.path());
        std::fs::create_dir_all(python.parent().expect("interpreter directory"))
            .expect("create fixture venv");
        std::fs::write(&python, "#!/bin/sh\nif [ \"$1\" = '-I' ]; then test -f \"$0.ready\"; exit $?; fi\nif [ \"$4\" = '--upgrade' ]; then exit 0; fi\necho $$ > \"$0.started\"\nif test -f \"$0.fail\"; then exit 1; fi\nwhile ! test -f \"$0.release\"; do sleep 0.02; done\ntouch \"$0.ready\"\n").expect("write fixture interpreter");
        std::fs::set_permissions(&python, std::fs::Permissions::from_mode(0o700))
            .expect("executable interpreter");
        root
    }

    fn marker(root: &Path, name: &str) -> PathBuf {
        scripts::venv_python(root).with_file_name(format!("python.{name}"))
    }

    async fn started(root: &Path) -> u32 {
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if let Ok(pid) = tokio::fs::read_to_string(marker(root, "started")).await {
                    if let Ok(pid) = pid.trim().parse() {
                        return pid;
                    }
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("fixture child starts")
    }

    async fn manager(root: PathBuf) -> super::super::ConversionManager {
        let library = Arc::new(
            crate::model_library::ModelLibrary::new(root.join("models"))
                .await
                .expect("fixture model library"),
        );
        let importer = Arc::new(crate::model_library::ModelImporter::new(Arc::clone(
            &library,
        )));
        super::super::ConversionManager::new(root, library, importer)
    }

    #[tokio::test]
    async fn dropped_waiter_retains_setup_and_alias_exclusion_until_shutdown() {
        let root = fixture();
        let owner = Arc::new(manager(root.path().to_path_buf()).await);
        let waiting = Arc::clone(&owner);
        let waiter = tokio::spawn(async move { waiting.ensure_environment().await });
        let pid = started(root.path()).await;
        let admitted_id = owner
            .get_conversion_setup()
            .expect("owned setup identity")
            .operation_id;
        waiter.abort();
        assert!(waiter.await.expect_err("waiter cancelled").is_cancelled());
        assert_eq!(
            owner
                .start_conversion_setup(None)
                .await
                .expect("inspect after waiter dropped")
                .operation_id,
            admitted_id
        );

        let aliases = tempfile::tempdir().expect("alias root");
        let alias = aliases.path().join("launcher");
        symlink(root.path(), &alias).expect("physical root alias");
        let contender = manager(alias).await;
        let requirements = scripts::scripts_dir(root.path()).join("requirements.txt");
        std::fs::write(&requirements, "unchanged while busy").expect("sentinel");
        std::fs::write(requirements.with_extension("txt.hash"), "invalid")
            .expect("invalidate script hash");
        assert!(contender
            .ensure_environment()
            .await
            .expect_err("root busy")
            .to_string()
            .contains("already running"));
        assert_eq!(
            std::fs::read_to_string(requirements).expect("read sentinel"),
            "unchanged while busy"
        );
        let subprocess_root = root.path().to_path_buf();
        let process = tokio::task::spawn_blocking(move || {
            Command::new(std::env::current_exe().expect("test executable"))
                .args([
                    "--exact",
                    "conversion::setup::tests::cross_process_lock_probe",
                ])
                .env("PUMAS_SETUP_LOCK_TEST_ROOT", subprocess_root)
                .output()
                .expect("launch independent lock probe")
        })
        .await
        .expect("join lock probe");
        assert!(
            process.status.success(),
            "{}",
            String::from_utf8_lossy(&process.stderr)
        );

        let mut interrupted_shutdown = Box::pin(owner.shutdown_setup());
        let _ = std::future::poll_fn(|cx| {
            std::task::Poll::Ready(interrupted_shutdown.as_mut().poll(cx))
        })
        .await;
        drop(interrupted_shutdown);
        tokio::time::timeout(Duration::from_secs(5), owner.shutdown_setup())
            .await
            .expect("cleanup bounded")
            .expect("cancel cleanup succeeds");
        owner.shutdown_setup().await.expect("idempotent shutdown");
        assert!(
            !Path::new(&format!("/proc/{pid}")).exists(),
            "direct child reaped"
        );
        assert!(
            !group_has_live_members(i32::try_from(pid).expect("pid fits"))
                .expect("observe descendants")
        );
        assert!(matches!(
            owner.ensure_environment().await,
            Err(PumasError::InstallationCancelled)
        ));
        assert!(
            acquire(root.path()).is_ok(),
            "lease released only after cleanup"
        );
    }

    #[test]
    fn cross_process_lock_probe() {
        let Some(root) = std::env::var_os("PUMAS_SETUP_LOCK_TEST_ROOT") else {
            return;
        };
        assert!(
            matches!(acquire(Path::new(&root)), Err(Failure::Failed(message)) if message.contains("already running"))
        );
    }

    #[test]
    fn setup_completes_with_one_host_blocking_thread() {
        let root = fixture();
        std::fs::write(marker(root.path(), "release"), "").expect("release fixture");
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .max_blocking_threads(1)
            .build()
            .expect("embedding runtime");
        let owner = SetupOwner::new(root.path().to_path_buf());
        let result = runtime.block_on(async {
            tokio::time::timeout(Duration::from_secs(5), async {
                owner.ensure().await.expect("setup succeeds");
                owner.shutdown().await.expect("setup drains");
            })
            .await
        });
        drop(owner);
        runtime.shutdown_timeout(Duration::from_secs(1));
        result.expect("no nested blocking-pool deadlock");
    }

    #[tokio::test]
    async fn overlapping_callers_share_completion_and_completed_failure_can_retry() {
        let root = fixture();
        let owner = Arc::new(SetupOwner::new(root.path().to_path_buf()));
        let first_owner = Arc::clone(&owner);
        let first = tokio::spawn(async move { first_owner.ensure().await });
        started(root.path()).await;
        let mut second = Box::pin(owner.ensure());
        assert!(
            std::future::poll_fn(|cx| std::task::Poll::Ready(second.as_mut().poll(cx)))
                .await
                .is_pending()
        );
        let operation = owner
            .state
            .lock()
            .expect("owner state")
            .operation
            .clone()
            .expect("active operation");
        std::fs::write(marker(root.path(), "release"), "").expect("release installer");
        first.await.expect("first waiter").expect("first success");
        second.await.expect("shared success");
        assert!(Arc::ptr_eq(
            &operation,
            owner
                .state
                .lock()
                .expect("state")
                .operation
                .as_ref()
                .expect("retained result")
        ));
        owner.shutdown().await.expect("success retained");

        let failed_root = fixture();
        std::fs::write(marker(failed_root.path(), "fail"), "").expect("fail installer");
        let failed_owner = SetupOwner::new(failed_root.path().to_path_buf());
        assert!(failed_owner.ensure().await.is_err());
        let failed_operation = failed_owner
            .state
            .lock()
            .expect("state")
            .operation
            .clone()
            .expect("failed result retained");
        std::fs::remove_file(marker(failed_root.path(), "fail")).expect("repair fixture");
        std::fs::write(marker(failed_root.path(), "release"), "").expect("release retry");
        failed_owner
            .ensure()
            .await
            .expect("explicit retry succeeds");
        assert!(
            failed_operation.observe().await.is_err(),
            "original failure stays observable"
        );
        failed_owner
            .shutdown()
            .await
            .expect("latest successful result");
    }

    #[test]
    fn released_setup_lease_allows_retry_with_an_inherited_descriptor() {
        let root = tempfile::tempdir().expect("isolated lease root");
        let (_, lease) = acquire(root.path()).expect("first setup lease");
        // dup and fork retain the same open-file description. Keep it alive
        // deterministically instead of racing an unrelated child's pre-exec.
        let inherited = lease
            .0
            .as_ref()
            .expect("active lease")
            .try_clone()
            .expect("duplicate inherited descriptor");
        assert!(
            acquire(root.path()).is_err(),
            "active setup excludes retries"
        );
        drop(lease);
        let successor = acquire(root.path()).expect("completed setup permits retry");
        drop(inherited);
        assert!(
            acquire(root.path()).is_err(),
            "successor still owns exclusion"
        );
        drop(successor);
        assert!(acquire(root.path()).is_ok(), "successor releases exclusion");
    }

    #[tokio::test]
    async fn symlinked_launcher_data_keeps_original_execution_paths() {
        let root = fixture();
        let storage = tempfile::tempdir().expect("physical storage");
        let actual = storage.path().join("tools");
        std::fs::rename(root.path().join("launcher-data"), &actual).expect("relocate data");
        symlink(&actual, root.path().join("launcher-data")).expect("link data");
        std::fs::write(marker(root.path(), "release"), "").expect("release fixture");
        let owner = SetupOwner::new(root.path().to_path_buf());
        owner.ensure().await.expect("setup through data symlink");
        assert!(marker(root.path(), "ready").exists());
        assert!(
            !storage.path().join("launcher-data").exists(),
            "no redirected execution"
        );
        owner.shutdown().await.expect("shutdown");
    }

    #[tokio::test]
    async fn worker_join_failure_is_retained_without_polling_a_finished_handle() {
        let (sender, receiver) = watch::channel(None);
        let handle = tokio::spawn(async move {
            drop(sender);
            panic!("controlled setup worker failure");
        });
        let operation = Operation {
            id: uuid::Uuid::new_v4().to_string(),
            failure: Mutex::new(None),
            cancel: CancellationToken::new(),
            completion: receiver,
            worker: tokio::sync::Mutex::new(WorkerReceipt {
                handle: Some(handle),
            }),
        };
        tokio::time::timeout(Duration::from_secs(5), async {
            while operation.snapshot().status == ConversionSetupStatus::InProgress {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("closed completion channel is not eternal in-progress");
        assert_eq!(operation.snapshot().status, ConversionSetupStatus::Failed);
        for _ in 0..2 {
            assert!(matches!(operation.observe().await,
                Err(Failure::Failed(message)) if message == "Conversion setup worker failed"));
            assert_eq!(operation.snapshot().status, ConversionSetupStatus::Failed);
        }
    }

    async fn terminal(manager: &super::super::ConversionManager) -> ConversionSetupSnapshot {
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let snapshot = manager.get_conversion_setup().expect("admitted operation");
                if snapshot.status != ConversionSetupStatus::InProgress {
                    return snapshot;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("setup reaches terminal state")
    }

    #[test]
    fn idle_snapshot_is_read_only_and_has_no_record() {
        let root = tempfile::tempdir().expect("temporary root");
        let absent = root.path().join("not-created");
        let owner = SetupOwner::new(absent.clone());
        assert_eq!(owner.snapshot(), None);
        assert!(!absent.exists());
    }

    #[tokio::test]
    async fn public_start_and_retry_are_identity_conditioned_and_cleanup_precedes_terminal() {
        let root = fixture();
        let manager = manager(root.path().to_path_buf()).await;
        assert_eq!(manager.get_conversion_setup(), None);
        let (first, repeated) = tokio::join!(
            manager.start_conversion_setup(None),
            manager.start_conversion_setup(None)
        );
        let first = first.expect("first admission");
        assert_eq!(
            first.operation_id,
            repeated.expect("joined admission").operation_id
        );
        assert_eq!(first.status, ConversionSetupStatus::InProgress);
        assert_eq!(first.error, None);
        assert_eq!(
            uuid::Uuid::parse_str(&first.operation_id)
                .expect("UUID identity")
                .to_string(),
            first.operation_id
        );
        started(root.path()).await;
        assert_eq!(
            manager
                .start_conversion_setup(Some(&first.operation_id))
                .await
                .expect("active retry is a read")
                .operation_id,
            first.operation_id
        );
        std::fs::write(marker(root.path(), "release"), "").expect("release installer");
        let completed = terminal(&manager).await;
        assert_eq!(completed.status, ConversionSetupStatus::Completed);
        assert_eq!(completed.error, None);
        assert!(
            acquire(root.path()).is_ok(),
            "terminal publication follows lease release"
        );
        assert_eq!(
            manager
                .start_conversion_setup(None)
                .await
                .expect("read completed")
                .operation_id,
            first.operation_id
        );
        let (retry, duplicate) = tokio::join!(
            manager.start_conversion_setup(Some(&first.operation_id)),
            manager.start_conversion_setup(Some(&first.operation_id)),
        );
        let retry = retry.expect("explicit successor");
        assert_ne!(retry.operation_id, first.operation_id);
        assert_eq!(
            duplicate.expect("CAS duplicate").operation_id,
            retry.operation_id
        );
        assert_eq!(
            terminal(&manager).await.status,
            ConversionSetupStatus::Completed
        );
        assert_eq!(
            manager
                .start_conversion_setup(Some(&first.operation_id))
                .await
                .expect("stale token is a read even after successor completion")
                .operation_id,
            retry.operation_id
        );
        manager.shutdown_setup().await.expect("drain setup");
        assert!(matches!(
            manager.start_conversion_setup(None).await,
            Err(PumasError::InstallationCancelled)
        ));
    }

    #[tokio::test]
    async fn failed_and_cancelled_setup_snapshots_retain_exact_error_invariant() {
        let root = fixture();
        std::fs::write(marker(root.path(), "fail"), "").expect("fail fixture installer");
        let manager = manager(root.path().to_path_buf()).await;
        let admitted = manager
            .start_conversion_setup(None)
            .await
            .expect("admit failed setup");
        let failed = terminal(&manager).await;
        assert_eq!(failed.status, ConversionSetupStatus::Failed);
        assert!(failed.error.as_ref().is_some_and(|error| !error.is_empty()));
        assert_eq!(failed.operation_id, admitted.operation_id);
        assert_eq!(
            manager
                .start_conversion_setup(None)
                .await
                .expect("no implicit retry"),
            failed
        );
        std::fs::remove_file(marker(root.path(), "fail")).expect("repair fixture");
        let retry = manager
            .start_conversion_setup(Some(&failed.operation_id))
            .await
            .expect("explicit retry");
        assert_ne!(retry.operation_id, failed.operation_id);
        // The old marker is not a readiness condition for the new child. The
        // queued/active setup is owned and cancellation must drain it either way.
        manager
            .shutdown_setup()
            .await
            .expect("cancel and drain retry");
        let cancelled = manager.get_conversion_setup().expect("cancelled record");
        assert_eq!(cancelled.operation_id, retry.operation_id);
        assert_eq!(cancelled.status, ConversionSetupStatus::Cancelled);
        assert_eq!(cancelled.error, None);
        assert!(acquire(root.path()).is_ok());
    }

    #[tokio::test]
    async fn unknown_or_invalid_retry_tokens_do_not_create_setup_state_or_files() {
        let root = tempfile::tempdir().expect("root");
        let absent = root.path().join("absent");
        let owner = SetupOwner::new(absent.clone());
        let old_id = uuid::Uuid::new_v4().to_string();
        for token in [
            "",
            "not-an-id",
            "00112233-4455-6677-8899-AABBCCDDEEFF",
            old_id.as_str(),
        ] {
            assert!(matches!(
                owner.start_or_get(Some(token)).await,
                Err(PumasError::InvalidParams { .. })
            ));
            assert_eq!(owner.snapshot(), None);
            assert!(!absent.exists());
        }
    }

    #[tokio::test]
    async fn command_deadline_and_unwind_reap_owned_children() {
        tokio::task::spawn_blocking(|| {
            let mut command = Command::new("sh");
            command.args(["-c", "while :; do sleep 1; done"]);
            assert!(run_command(
                &mut command,
                &CancellationToken::new(),
                Duration::from_millis(50)
            )
            .is_err());
            use std::os::unix::process::CommandExt;
            let child = Command::new("sh")
                .args(["-c", "while :; do sleep 1; done"])
                .process_group(0)
                .spawn()
                .expect("panic fixture");
            let pid = child.id();
            let panic = std::panic::catch_unwind(|| {
                let _owned = OwnedChild(Some(child));
                panic!("controlled custody unwind");
            });
            assert!(panic.is_err());
            assert!(!Path::new(&format!("/proc/{pid}")).exists());
            assert!(
                !group_has_live_members(i32::try_from(pid).expect("pid fits"))
                    .expect("observe group")
            );
        })
        .await
        .expect("join controlled command tests");
    }

    #[test]
    fn successful_setup_command_cleans_group_before_reaping_leader() {
        let root = tempfile::tempdir().expect("fixture root");
        let marker = root.path().join("group");
        let mut command = Command::new("sh");
        command
            .args([
                "-c",
                "printf '%s\\n' \"$$\" > \"$1\"; sleep 30 & exit 0",
                "fixture",
            ])
            .arg(&marker);
        let status = run_command(
            &mut command,
            &CancellationToken::new(),
            Duration::from_secs(5),
        )
        .expect("successful setup command");
        assert!(status.success());
        let group: i32 = std::fs::read_to_string(marker)
            .expect("group identity")
            .trim()
            .parse()
            .expect("numeric group");
        assert!(!group_has_live_members(group).expect("no live group after normal completion"));
        assert!(
            !Path::new(&format!("/proc/{group}")).exists(),
            "leader reaped after group observation"
        );
    }
}
