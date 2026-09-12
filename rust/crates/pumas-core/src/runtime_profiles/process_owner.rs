//! Session custody for managed binary profiles. PID files are diagnostics, never
//! authority to adopt or signal a process. Linux leaders remain unreaped until
//! cooperating group cleanup completes; escaped descendants are not contained.

use super::router_model_operation::{OwnedRouterModelOperation, RouterModelState};
use super::{RuntimeProfileLaunchSpec, RuntimeProfileOperationGuard};
use crate::models::{
    LaunchResponse, RuntimeEndpointUrl, RuntimeLifecycleState, RuntimeProfileId,
    RuntimeProfileStatus,
};
use crate::process::BinaryLaunchConfig;
use crate::{PumasError, Result};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Child;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::task::JoinHandle;

const OBSERVATION_INTERVAL: Duration = Duration::from_millis(20);
const STOP_WAIT: Duration = Duration::from_secs(7);

/// In-memory identity of a process launched by this API instance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnedRuntimeProfileObservation {
    pub generation: u64,
    pub pid: Option<u32>,
    pub state: RuntimeLifecycleState,
    pub endpoint_url: RuntimeEndpointUrl,
    pub model_path: Option<PathBuf>,
    /// Explicit launch override; absence leaves runtime default selection intact.
    pub context_size: Option<u32>,
}

/// Receipt bound to the exact session admitted by a launch call.
#[derive(Debug, Clone)]
pub struct OwnedRuntimeProfileLaunchReceipt {
    pub response: LaunchResponse,
    pub observation: Option<OwnedRuntimeProfileObservation>,
}

#[derive(Debug, Default)]
pub(crate) struct RuntimeProfileProcessOwner {
    registry: Mutex<Registry>,
}

#[derive(Debug, Default)]
struct Registry {
    closed: bool,
    #[cfg(target_os = "linux")]
    generation: u64,
    sessions: HashMap<RuntimeProfileId, Arc<Session>>,
}

#[derive(Debug)]
struct Session {
    spec: RuntimeProfileLaunchSpec,
    generation: u64,
    model_path: Option<PathBuf>,
    context_size: Option<u32>,
    router_models: Option<Arc<Mutex<RouterModelState>>>,
    stop: AtomicBool,
    state: Mutex<SessionState>,
}

#[derive(Debug)]
struct SessionState {
    status: RuntimeProfileStatus,
    #[cfg(target_os = "linux")]
    launch: Option<std::result::Result<OwnedRuntimeProfileObservation, String>>,
    terminal: Option<std::result::Result<bool, String>>,
    worker: Option<JoinHandle<()>>,
    joined: bool,
    // A failed cleanup never releases child custody or permits replacement.
    residual_child: Option<Child>,
}

fn failure(message: impl Into<String>) -> PumasError {
    PumasError::Other(message.into())
}

impl RuntimeProfileProcessOwner {
    pub(crate) fn ensure_inactive(&self, profile_id: &RuntimeProfileId) -> Result<()> {
        let registry = self
            .registry
            .lock()
            .map_err(|_| failure("Runtime process registry poisoned"))?;
        if registry.closed {
            return Err(failure("Managed runtime admission is closed"));
        }
        if let Some(session) = registry.sessions.get(profile_id) {
            let state = session
                .state
                .lock()
                .map_err(|_| failure("Runtime process session poisoned"))?;
            if state.terminal.is_none() || state.residual_child.is_some() || !state.joined {
                return Err(failure(format!(
                    "Managed runtime profile {} still owns a process or worker",
                    profile_id.as_str()
                )));
            }
        }
        Ok(())
    }

    pub(crate) async fn launch(
        &self,
        config: BinaryLaunchConfig,
        spec: RuntimeProfileLaunchSpec,
        model_path: Option<PathBuf>,
        context_size: Option<u32>,
        guard: RuntimeProfileOperationGuard,
    ) -> Result<OwnedRuntimeProfileLaunchReceipt> {
        #[cfg(not(target_os = "linux"))]
        {
            let _ = (config, spec, model_path, context_size, guard);
            Err(failure(
                "Owned binary runtime profiles are currently supported only on Linux",
            ))
        }
        #[cfg(target_os = "linux")]
        {
            crate::platform::linux_group::ensure_supported().map_err(|e| failure(e.to_string()))?;
            let router_models = RouterModelState::for_spec(&spec);
            let session = {
                let mut registry = self
                    .registry
                    .lock()
                    .map_err(|_| failure("Runtime process registry poisoned"))?;
                if registry.closed {
                    return Err(failure("Managed runtime admission is closed"));
                }
                if let Some(previous) = registry.sessions.get(&spec.profile_id) {
                    let state = previous
                        .state
                        .lock()
                        .map_err(|_| failure("Runtime process session poisoned"))?;
                    if state.terminal.is_none() || state.residual_child.is_some() || !state.joined {
                        return Err(failure(
                            "Managed runtime profile already owns a process or worker",
                        ));
                    }
                }
                registry.generation = registry
                    .generation
                    .checked_add(1)
                    .ok_or_else(|| failure("Runtime generation exhausted"))?;
                let session = Arc::new(Session {
                    generation: registry.generation,
                    router_models,
                    model_path,
                    context_size,
                    stop: AtomicBool::new(false),
                    state: Mutex::new(SessionState {
                        status: RuntimeProfileStatus {
                            profile_id: spec.profile_id.clone(),
                            state: RuntimeLifecycleState::Starting,
                            endpoint_url: Some(spec.endpoint_url.clone()),
                            pid: None,
                            log_path: config
                                .log_file
                                .as_ref()
                                .map(|p| p.to_string_lossy().into_owned()),
                            last_error: None,
                        },
                        launch: None,
                        terminal: None,
                        worker: None,
                        joined: false,
                        residual_child: None,
                    }),
                    spec,
                });
                registry
                    .sessions
                    .insert(session.spec.profile_id.clone(), session.clone());
                session
            };
            // No await between registration, worker retention and gate release.
            let (start, gate) = std::sync::mpsc::channel();
            let worker_session = session.clone();
            let worker = tokio::task::spawn_blocking(move || {
                if gate.recv().is_ok() {
                    run_worker(&worker_session, config, guard);
                }
            });
            session
                .state
                .lock()
                .map_err(|_| failure("Runtime process session poisoned"))?
                .worker = Some(worker);
            start
                .send(())
                .map_err(|_| failure("Runtime process start gate closed"))?;
            loop {
                let launch = session
                    .state
                    .lock()
                    .map_err(|_| failure("Runtime process session poisoned"))?
                    .launch
                    .clone();
                if let Some(outcome) = launch {
                    let response = LaunchResponse {
                        success: outcome.is_ok(),
                        error: outcome.as_ref().err().cloned(),
                        log_path: Some(session.spec.log_file.to_string_lossy().into_owned()),
                        ready: Some(false),
                    };
                    return Ok(OwnedRuntimeProfileLaunchReceipt {
                        response,
                        observation: outcome.ok(),
                    });
                }
                tokio::time::sleep(OBSERVATION_INTERVAL).await;
            }
        }
    }

    pub(crate) fn snapshot(
        &self,
        id: &RuntimeProfileId,
    ) -> Result<Option<OwnedRuntimeProfileObservation>> {
        let registry = self
            .registry
            .lock()
            .map_err(|_| failure("Runtime process registry poisoned"))?;
        registry
            .sessions
            .get(id)
            .map(|session| {
                let state = session
                    .state
                    .lock()
                    .map_err(|_| failure("Runtime process session poisoned"))?;
                let mut lifecycle = state.status.state;
                if session.stop.load(Ordering::Acquire)
                    && matches!(
                        lifecycle,
                        RuntimeLifecycleState::Starting | RuntimeLifecycleState::Running
                    )
                {
                    lifecycle = RuntimeLifecycleState::Stopping;
                }
                #[cfg(target_os = "linux")]
                if lifecycle == RuntimeLifecycleState::Running {
                    if let Some(pid) = state.status.pid {
                        if crate::platform::linux_group::observe_exit(pid)
                            .map_err(|e| failure(format!("Owned runtime observation failed: {e}")))?
                            .is_some()
                        {
                            lifecycle = RuntimeLifecycleState::Stopping;
                        }
                    }
                }
                Ok(OwnedRuntimeProfileObservation {
                    generation: session.generation,
                    pid: state.status.pid,
                    state: lifecycle,
                    endpoint_url: session.spec.endpoint_url.clone(),
                    model_path: session.model_path.clone(),
                    context_size: session.context_size,
                })
            })
            .transpose()
    }

    pub(crate) fn owns_current_listener(
        &self,
        id: &RuntimeProfileId,
        expected: &OwnedRuntimeProfileObservation,
    ) -> Result<bool> {
        Ok(self.with_listener(id, expected, |_| ())?.is_some())
    }

    pub(crate) fn with_running_session<T>(
        &self,
        id: &RuntimeProfileId,
        expected: &OwnedRuntimeProfileObservation,
        publish: impl FnOnce() -> T,
    ) -> Result<T> {
        self.with_listener(id, expected, |_| publish())?
            .ok_or_else(|| failure("Runtime endpoint listener is not owned by the admitted child"))
    }

    pub(crate) fn begin_router_model_operation(
        self: &Arc<Self>,
        id: &RuntimeProfileId,
        expected: &OwnedRuntimeProfileObservation,
    ) -> Result<OwnedRouterModelOperation> {
        self.with_listener(id, expected, |session| {
            let models = session.router_models.clone().ok_or_else(|| {
                failure("Model operations require an owned llama.cpp router session")
            })?;
            OwnedRouterModelOperation::begin(self.clone(), id.clone(), expected.clone(), models)
        })?
        .ok_or_else(|| failure("Runtime endpoint listener is not owned by the admitted child"))?
    }

    fn with_listener<T>(
        &self,
        id: &RuntimeProfileId,
        expected: &OwnedRuntimeProfileObservation,
        publish: impl FnOnce(&Session) -> T,
    ) -> Result<Option<T>> {
        let registry = self
            .registry
            .lock()
            .map_err(|_| failure("Runtime process registry poisoned"))?;
        if registry.closed {
            return Err(failure("Managed runtime admission is closed"));
        }
        let session = registry
            .sessions
            .get(id)
            .ok_or_else(|| failure("Managed session is absent"))?;
        let state = session
            .state
            .lock()
            .map_err(|_| failure("Runtime process session poisoned"))?;
        if session.stop.load(Ordering::Acquire)
            || state.status.state != RuntimeLifecycleState::Running
            || session.generation != expected.generation
            || state.status.pid != expected.pid
            || session.model_path != expected.model_path
            || session.context_size != expected.context_size
            || session.spec.endpoint_url != expected.endpoint_url
        {
            return Err(failure(
                "Managed runtime receipt is no longer current and running",
            ));
        }
        #[cfg(target_os = "linux")]
        {
            let pid = state
                .status
                .pid
                .ok_or_else(|| failure("Managed runtime PID is absent"))?;
            if crate::platform::linux_group::observe_exit(pid)
                .map_err(|e| failure(e.to_string()))?
                .is_some()
            {
                return Err(failure("Managed runtime has exited"));
            }
            if !crate::platform::runtime_listener::owns_listener(pid, &session.spec.endpoint_url)
                .map_err(|e| failure(format!("Runtime listener attribution unavailable: {e}")))?
            {
                return Ok(None);
            }
            Ok(Some(publish(session)))
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = publish;
            Err(failure(
                "Owned runtime publication unsupported on this platform",
            ))
        }
    }

    pub(crate) fn statuses(&self) -> Result<Vec<RuntimeProfileStatus>> {
        let registry = self
            .registry
            .lock()
            .map_err(|_| failure("Runtime process registry poisoned"))?;
        registry
            .sessions
            .values()
            .map(|session| {
                session
                    .state
                    .lock()
                    .map(|s| s.status.clone())
                    .map_err(|_| failure("Runtime process session poisoned"))
            })
            .collect()
    }

    #[cfg(test)]
    pub(crate) async fn stop(&self, id: &RuntimeProfileId) -> Result<bool> {
        match self.stop_with_receipt(id).await? {
            Some((_, result)) => result,
            None => Ok(false),
        }
    }

    pub(crate) async fn stop_with_receipt(
        &self,
        id: &RuntimeProfileId,
    ) -> Result<Option<(OwnedRuntimeProfileObservation, Result<bool>)>> {
        let session = self
            .registry
            .lock()
            .map_err(|_| failure("Runtime process registry poisoned"))?
            .sessions
            .get(id)
            .cloned();
        let Some(session) = session else {
            return Ok(None);
        };
        let receipt = {
            let state = session
                .state
                .lock()
                .map_err(|_| failure("Runtime process session poisoned"))?;
            session.stop.store(true, Ordering::Release);
            OwnedRuntimeProfileObservation {
                generation: session.generation,
                pid: state.status.pid,
                state: state.status.state,
                endpoint_url: session.spec.endpoint_url.clone(),
                model_path: session.model_path.clone(),
                context_size: session.context_size,
            }
        };
        Ok(Some((receipt, drain_session(&session).await)))
    }

    pub(crate) fn with_current_generation<T>(
        &self,
        id: &RuntimeProfileId,
        generation: u64,
        publish: impl FnOnce() -> T,
    ) -> Result<Option<T>> {
        let registry = self
            .registry
            .lock()
            .map_err(|_| failure("Runtime process registry poisoned"))?;
        if registry
            .sessions
            .get(id)
            .is_some_and(|session| session.generation == generation)
        {
            Ok(Some(publish()))
        } else {
            Ok(None)
        }
    }

    pub(crate) async fn close_and_drain(&self) -> Result<Vec<(RuntimeProfileId, Result<bool>)>> {
        let sessions = {
            let mut registry = self
                .registry
                .lock()
                .map_err(|_| failure("Runtime process registry poisoned"))?;
            registry.closed = true;
            registry.sessions.values().cloned().collect::<Vec<_>>()
        };
        for session in &sessions {
            let _state = session
                .state
                .lock()
                .map_err(|_| failure("Runtime process session poisoned"))?;
            session.stop.store(true, Ordering::Release);
        }
        let mut results = Vec::new();
        for session in sessions {
            results.push((
                session.spec.profile_id.clone(),
                drain_session(&session).await,
            ));
        }
        Ok(results)
    }
}

async fn drain_session(session: &Session) -> Result<bool> {
    let deadline = tokio::time::Instant::now() + STOP_WAIT;
    loop {
        let worker = {
            let mut state = session
                .state
                .lock()
                .map_err(|_| failure("Runtime process session poisoned"))?;
            if state.worker.as_ref().is_some_and(JoinHandle::is_finished) {
                state.worker.take()
            } else {
                None
            }
        };
        if let Some(worker) = worker {
            // is_finished proves this await cannot leave an unobserved running worker.
            let outcome = worker.await;
            let mut state = session
                .state
                .lock()
                .map_err(|_| failure("Runtime process session poisoned"))?;
            if let Err(error) = outcome {
                state.terminal = Some(Err(format!("Runtime worker failed: {error}")));
                state.status.state = RuntimeLifecycleState::Failed;
            }
            state.joined = true;
        }
        {
            let state = session
                .state
                .lock()
                .map_err(|_| failure("Runtime process session poisoned"))?;
            if state.joined {
                if let Some(result) = &state.terminal {
                    return result.clone().map_err(failure);
                }
            }
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(failure(
                "Runtime cleanup remains unobserved; session custody retained",
            ));
        }
        tokio::time::sleep(OBSERVATION_INTERVAL).await;
    }
}

#[cfg(target_os = "linux")]
fn run_worker(session: &Session, config: BinaryLaunchConfig, guard: RuntimeProfileOperationGuard) {
    let mut child = None;
    let mut pid_file = None;
    let execution = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> Result<()> {
        use std::io::Write;
        use std::os::unix::process::CommandExt;
        use std::process::{Command, Stdio};
        if let Some(models) = &session.router_models {
            RouterModelState::capture(models)?;
        }
        // Reserve diagnostic metadata without adopting or replacing existing state.
        if let Some(parent) = config.pid_file.parent() {
            std::fs::create_dir_all(parent).map_err(|e| PumasError::io_with_path(e, parent))?;
        }
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&config.pid_file)
            .map_err(|e| {
                failure(format!(
                    "Cannot reserve managed runtime PID metadata (existing files are unowned): {e}"
                ))
            })?;
        pid_file = Some((
            config.pid_file.clone(),
            file.metadata()
                .map_err(|e| PumasError::io_with_path(e, &config.pid_file))?,
        ));
        let mut command = Command::new(&config.binary_path);
        if let Some(arg) = &config.command {
            command.arg(arg);
        }
        command
            .args(&config.extra_args)
            .envs(&config.env_vars)
            .current_dir(&config.version_dir)
            .stdin(Stdio::null())
            .process_group(0);
        if let Some(path) = &config.log_file {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| PumasError::io_with_path(e, parent))?;
            }
            let file =
                std::fs::File::create(path).map_err(|e| PumasError::io_with_path(e, path))?;
            command
                .stdout(
                    file.try_clone()
                        .map_err(|e| PumasError::io_with_path(e, path))?,
                )
                .stderr(file);
        } else {
            command.stdout(Stdio::null()).stderr(Stdio::null());
        }
        if session.stop.load(Ordering::Acquire) {
            return Err(failure("Runtime launch stopped before process creation"));
        }
        child = Some(
            command
                .spawn()
                .map_err(|e| failure(format!("Runtime spawn failed: {e}")))?,
        );
        let pid = child.as_ref().expect("spawned child").id();
        write!(file, "{pid}").map_err(|e| PumasError::io_with_path(e, &config.pid_file))?;
        {
            let mut state = session
                .state
                .lock()
                .map_err(|_| failure("Runtime process session poisoned"))?;
            state.status.pid = Some(pid);
            state.status.state = RuntimeLifecycleState::Running;
            state.launch = Some(Ok(OwnedRuntimeProfileObservation {
                generation: session.generation,
                pid: Some(pid),
                state: RuntimeLifecycleState::Running,
                endpoint_url: session.spec.endpoint_url.clone(),
                model_path: session.model_path.clone(),
                context_size: session.context_size,
            }));
        }
        drop(guard);
        loop {
            if let Some(models) = &session.router_models {
                RouterModelState::process_pending(models, |receipt| {
                    if session.stop.load(Ordering::Acquire)
                        || receipt.generation != session.generation
                        || receipt.pid != Some(pid)
                        || receipt.endpoint_url != session.spec.endpoint_url
                        || receipt.context_size != session.context_size
                        || receipt.model_path != session.model_path
                    {
                        return Err(failure(
                            "Managed runtime receipt is no longer current and running",
                        ));
                    }
                    if crate::platform::linux_group::observe_exit(pid)
                        .map_err(|e| failure(e.to_string()))?
                        .is_some()
                        || !crate::platform::runtime_listener::owns_listener(
                            pid,
                            &session.spec.endpoint_url,
                        )
                        .map_err(|e| {
                            failure(format!("Runtime listener attribution unavailable: {e}"))
                        })?
                    {
                        return Err(failure(
                            "Runtime endpoint listener is not owned by the admitted child",
                        ));
                    }
                    Ok(())
                })?;
            }
            if session.stop.load(Ordering::Acquire) {
                return Ok(());
            }
            if let Some(status) = crate::platform::linux_group::observe_exit(pid)
                .map_err(|e| failure(format!("Runtime exit observation failed: {e}")))?
            {
                return Err(failure(format!("Managed runtime exited: {status}")));
            }
            std::thread::sleep(OBSERVATION_INTERVAL);
        }
    }));
    let mut error = match execution {
        Ok(Ok(())) => None,
        Ok(Err(e)) => Some(e.to_string()),
        Err(_) => Some("Managed runtime worker panicked".to_string()),
    };
    if let Ok(mut state) = session.state.lock() {
        state.status.state = RuntimeLifecycleState::Stopping;
    }
    if let Some(models) = &session.router_models {
        RouterModelState::reject_pending(models);
    }
    let spawned = child.is_some();
    if let Some(process) = child.as_mut() {
        if let Err(e) = cleanup_child(process) {
            error = Some(format!(
                "{}; cleanup failed: {e}",
                error.unwrap_or_default()
            ));
        } else {
            child = None;
        }
    }
    if child.is_none() {
        if let Some((path, owned_metadata)) = pid_file {
            use std::os::unix::fs::MetadataExt;
            let cleanup = std::fs::symlink_metadata(&path).and_then(|current| {
                if current.dev() != owned_metadata.dev() || current.ino() != owned_metadata.ino() {
                    return Err(std::io::Error::other(
                        "Runtime PID metadata was replaced; preserving unowned replacement",
                    ));
                }
                std::fs::remove_file(&path)
            });
            if let Err(e) = cleanup {
                error = Some(format!("Runtime PID metadata cleanup failed: {e}"));
            }
        }
    }
    if let Ok(mut state) = session.state.lock() {
        if state.launch.is_none() {
            state.launch = Some(Err(error
                .clone()
                .unwrap_or_else(|| "Runtime launch stopped".to_string())));
        }
        state.status.state = if error.is_some() {
            RuntimeLifecycleState::Failed
        } else {
            RuntimeLifecycleState::Stopped
        };
        state.status.pid = child.as_ref().map(Child::id);
        state.status.last_error = error.clone();
        state.residual_child = child;
        state.terminal = Some(match error {
            Some(error) => Err(error),
            None => Ok(spawned),
        });
    }
}

#[cfg(target_os = "linux")]
fn cleanup_child(child: &mut Child) -> Result<()> {
    use crate::platform::linux_group;
    linux_group::signal_group(child.id())
        .map_err(|e| failure(format!("Owned group signal failed: {e}")))?;
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    let group = i32::try_from(child.id()).map_err(|e| failure(e.to_string()))?;
    loop {
        if !linux_group::group_has_live_members(group)
            .map_err(|e| failure(format!("Owned group observation failed: {e}")))?
        {
            child
                .wait()
                .map_err(|e| failure(format!("Owned leader reap failed: {e}")))?;
            return Ok(());
        }
        if std::time::Instant::now() >= deadline {
            return Err(failure("Owned group cleanup deadline elapsed"));
        }
        std::thread::sleep(OBSERVATION_INTERVAL);
    }
}

#[cfg(all(test, target_os = "linux", not(target_env = "uclibc")))]
mod tests {
    use super::*;
    use crate::models::{RuntimePort, RuntimeProviderId, RuntimeProviderMode};
    use crate::runtime_profiles::{RuntimeProfileBinaryLaunchKind, RuntimeProfileLaunchStrategy};
    use std::collections::HashSet;

    struct Fixture {
        owner: Arc<RuntimeProfileProcessOwner>,
        root: tempfile::TempDir,
    }
    impl Fixture {
        fn new() -> Self {
            Self {
                owner: Arc::new(RuntimeProfileProcessOwner::default()),
                root: tempfile::tempdir().unwrap(),
            }
        }
        fn launch(
            &self,
            script: &str,
        ) -> (
            BinaryLaunchConfig,
            RuntimeProfileLaunchSpec,
            RuntimeProfileOperationGuard,
        ) {
            let root = self.root.path();
            let id = RuntimeProfileId::parse("controlled-fixture").unwrap();
            let endpoint = RuntimeEndpointUrl::parse("http://127.0.0.1:39123").unwrap();
            let spec = RuntimeProfileLaunchSpec {
                profile_id: id.clone(),
                provider: RuntimeProviderId::LlamaCpp,
                provider_mode: RuntimeProviderMode::LlamaCppDedicated,
                launch_strategy: RuntimeProfileLaunchStrategy::BinaryProcess(
                    RuntimeProfileBinaryLaunchKind::LlamaCppDedicated,
                ),
                endpoint_url: endpoint.clone(),
                port: RuntimePort::parse(39123).unwrap(),
                extra_args: vec![],
                env_vars: HashMap::new(),
                runtime_dir: root.to_path_buf(),
                pid_file: root.join("runtime.pid"),
                log_file: root.join("runtime.log"),
                health_check_url: endpoint,
            };
            let config = BinaryLaunchConfig {
                tag: "fixture".into(),
                version_dir: root.to_path_buf(),
                binary_path: PathBuf::from("/bin/sh"),
                command: None,
                extra_args: vec!["-c".into(), script.into()],
                env_vars: HashMap::new(),
                pid_file: spec.pid_file.clone(),
                log_file: Some(spec.log_file.clone()),
                ready_timeout: Duration::ZERO,
                health_check_url: None,
            };
            let guard = RuntimeProfileOperationGuard {
                profile_id: id.clone(),
                operation_locks: Arc::new(Mutex::new(HashSet::from([id]))),
            };
            (config, spec, guard)
        }
    }
    fn listener_launch(
        fixture: &Fixture,
        address: std::net::SocketAddr,
    ) -> (
        BinaryLaunchConfig,
        RuntimeProfileLaunchSpec,
        RuntimeProfileOperationGuard,
    ) {
        let (mut config, mut spec, guard) = fixture.launch("");
        config.binary_path = std::env::current_exe().unwrap();
        config.extra_args = vec![
            "--exact".into(),
            "runtime_profiles::process_owner::tests::listener_fixture_entry".into(),
            "--nocapture".into(),
        ];
        config
            .env_vars
            .insert("PUMAS_OWNED_LISTENER_FIXTURE".into(), address.to_string());
        spec.endpoint_url = RuntimeEndpointUrl::parse(format!("http://{address}")).unwrap();
        spec.port = RuntimePort::parse(address.port()).unwrap();
        (config, spec, guard)
    }

    #[test]
    fn listener_fixture_entry() {
        let Ok(address) = std::env::var("PUMAS_OWNED_LISTENER_FIXTURE") else {
            return;
        };
        let _listener = std::net::TcpListener::bind(address).unwrap();
        std::thread::sleep(Duration::from_secs(30));
    }

    async fn wait_for_listener(receipt: &OwnedRuntimeProfileObservation) {
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                if crate::platform::runtime_listener::owns_listener(
                    receipt.pid.unwrap(),
                    &receipt.endpoint_url,
                )
                .unwrap()
                {
                    break;
                }
                tokio::time::sleep(OBSERVATION_INTERVAL).await;
            }
        })
        .await
        .unwrap();
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            if let Ok(registry) = self.owner.registry.lock() {
                for session in registry.sessions.values() {
                    session.stop.store(true, Ordering::Release);
                }
            }
        }
    }

    #[tokio::test]
    async fn owned_launch_duplicate_admission_and_observed_stop() {
        let fixture = Fixture::new();
        let (config, spec, guard) = fixture.launch("sleep 30 & wait");
        let id = spec.profile_id.clone();
        let result = fixture
            .owner
            .launch(
                config,
                spec,
                Some(PathBuf::from("/fixture/model.gguf")),
                Some(8192),
                guard,
            )
            .await
            .unwrap();
        assert!(result.response.success);
        assert_eq!(result.response.ready, Some(false));
        let identity = fixture.owner.snapshot(&id).unwrap().unwrap();
        assert!(identity.pid.is_some());
        assert_eq!(identity.context_size, Some(8192));
        assert_eq!(
            result.observation.as_ref().unwrap().context_size,
            Some(8192)
        );
        assert_eq!(
            identity.model_path,
            Some(PathBuf::from("/fixture/model.gguf"))
        );
        let (config, spec, guard) = fixture.launch("exit 99");
        assert!(fixture
            .owner
            .launch(config, spec, None, None, guard)
            .await
            .is_err());
        assert_eq!(
            fixture.owner.snapshot(&id).unwrap().unwrap().generation,
            identity.generation
        );
        assert!(fixture.owner.stop(&id).await.unwrap());
        assert!(fixture.owner.stop(&id).await.unwrap());
        assert!(
            crate::platform::linux_group::observe_exit(identity.pid.unwrap()).is_err(),
            "leader reaped only after group cleanup"
        );
        assert!(!crate::platform::linux_group::group_has_live_members(
            i32::try_from(identity.pid.unwrap()).unwrap()
        )
        .unwrap());
        assert!(!fixture.root.path().join("runtime.pid").exists());
        assert_eq!(
            fixture.owner.snapshot(&id).unwrap().unwrap().state,
            RuntimeLifecycleState::Stopped
        );
    }

    #[tokio::test]
    async fn unowned_pid_file_is_preserved_without_signalling() {
        let fixture = Fixture::new();
        let mut unrelated = std::process::Command::new("sleep")
            .arg("30")
            .spawn()
            .unwrap();
        let pid_text = unrelated.id().to_string();
        std::fs::write(fixture.root.path().join("runtime.pid"), &pid_text).unwrap();
        let (config, spec, guard) = fixture.launch("exit 99");
        let id = spec.profile_id.clone();
        let result = fixture
            .owner
            .launch(config, spec, None, None, guard)
            .await
            .unwrap();
        assert!(!result.response.success);
        assert!(unrelated.try_wait().unwrap().is_none());
        assert_eq!(
            std::fs::read_to_string(fixture.root.path().join("runtime.pid")).unwrap(),
            pid_text
        );
        assert!(fixture.owner.stop(&id).await.is_err());
        unrelated.kill().unwrap();
        unrelated.wait().unwrap();
    }

    #[tokio::test]
    async fn cancelled_launch_waiter_leaves_owned_process_for_shutdown() {
        let fixture = Fixture::new();
        let (config, spec, guard) = fixture.launch("sleep 30 & wait");
        let id = spec.profile_id.clone();
        let owner = fixture.owner.clone();
        let waiter =
            tokio::spawn(async move { owner.launch(config, spec, None, None, guard).await });
        tokio::time::timeout(Duration::from_secs(3), async {
            while fixture.owner.snapshot(&id).unwrap().is_none() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        waiter.abort();
        let _ = waiter.await;
        let results = fixture.owner.close_and_drain().await.unwrap();
        assert_eq!(results.len(), 1);
        // Shutdown may win before spawn; either outcome must retain and observe cleanup.
        assert!(fixture.owner.snapshot(&id).unwrap().unwrap().pid.is_none());
        {
            let registry = fixture.owner.registry.lock().unwrap();
            let state = registry.sessions[&id].state.lock().unwrap();
            assert!(state.worker.is_none());
            assert!(state.residual_child.is_none());
        }
        let (config, spec, guard) = fixture.launch("exit 99");
        assert!(fixture
            .owner
            .launch(config, spec, None, None, guard)
            .await
            .is_err());
    }

    #[tokio::test]
    async fn natural_exit_cleans_descendants_and_preserves_failed_receipt() {
        let fixture = Fixture::new();
        let (config, spec, guard) = fixture.launch("sleep 30 & exit 7");
        let id = spec.profile_id.clone();
        fixture
            .owner
            .launch(config, spec, None, None, guard)
            .await
            .unwrap();
        tokio::time::timeout(Duration::from_secs(6), async {
            while fixture.owner.snapshot(&id).unwrap().unwrap().state
                != RuntimeLifecycleState::Failed
            {
                tokio::time::sleep(OBSERVATION_INTERVAL).await;
            }
        })
        .await
        .unwrap();
        assert!(fixture.owner.stop(&id).await.is_err());
        assert!(fixture.owner.snapshot(&id).unwrap().unwrap().pid.is_none());
        assert!(!fixture.root.path().join("runtime.pid").exists());
    }
    #[tokio::test]
    async fn serving_publication_rejects_stopped_and_replaced_receipts() {
        use crate::models::{RuntimeDeviceMode, ServedModelLoadState, ServedModelStatus};
        let fixture = Fixture::new();
        let serving = crate::serving::ServingService::with_provider_registry(
            crate::providers::ProviderRegistry::builtin(),
        );
        let foreign_listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = foreign_listener.local_addr().unwrap();
        let (config, mut spec, guard) = fixture.launch("sleep 30");
        spec.endpoint_url = RuntimeEndpointUrl::parse(format!("http://{address}")).unwrap();
        let id = spec.profile_id.clone();
        let receipt = fixture
            .owner
            .launch(config, spec, None, None, guard)
            .await
            .unwrap()
            .observation
            .unwrap();
        let status = ServedModelStatus {
            model_id: "fixture/model".into(),
            model_alias: None,
            provider: RuntimeProviderId::LlamaCpp,
            profile_id: id.clone(),
            load_state: ServedModelLoadState::Loaded,
            device_mode: RuntimeDeviceMode::Cpu,
            device_id: None,
            gpu_layers: None,
            tensor_split: None,
            context_size: None,
            keep_loaded: true,
            endpoint_url: None,
            memory_bytes: None,
            loaded_at: None,
            last_error: None,
        };
        assert!(
            serving
                .record_loaded_model_for_owned_profile(status.clone(), &fixture.owner, &receipt)
                .await
                .is_err(),
            "foreign parent listener cannot supply owned readiness"
        );
        assert!(serving.status().await.snapshot.served_models.is_empty());
        fixture.owner.stop(&id).await.unwrap();
        drop(foreign_listener);
        let (config, spec, guard) = listener_launch(&fixture, address);
        let receipt = fixture
            .owner
            .launch(config, spec, None, None, guard)
            .await
            .unwrap()
            .observation
            .unwrap();
        wait_for_listener(&receipt).await;
        assert_eq!(receipt.context_size, None);
        let mut mismatched_context = receipt.clone();
        mismatched_context.context_size = Some(1024);
        assert!(fixture
            .owner
            .owns_current_listener(&id, &mismatched_context)
            .is_err());
        assert!(serving
            .record_loaded_model_for_owned_profile(
                status.clone(),
                &fixture.owner,
                &mismatched_context
            )
            .await
            .is_err());
        assert!(serving.status().await.snapshot.served_models.is_empty());
        let loaded = serving
            .record_loaded_model_for_owned_profile(status.clone(), &fixture.owner, &receipt)
            .await
            .unwrap();
        assert_eq!(loaded.served_models.len(), 1);
        fixture.owner.stop(&id).await.unwrap();
        assert!(serving
            .record_loaded_model_for_owned_profile(status.clone(), &fixture.owner, &receipt)
            .await
            .is_err());
        assert_eq!(serving.status().await.snapshot, loaded);
        let (config, spec, guard) = listener_launch(&fixture, address);
        let replacement = fixture
            .owner
            .launch(config, spec, None, None, guard)
            .await
            .unwrap()
            .observation
            .unwrap();
        assert_ne!(replacement.generation, receipt.generation);
        wait_for_listener(&replacement).await;
        serving
            .record_loaded_model_for_owned_profile(status.clone(), &fixture.owner, &replacement)
            .await
            .unwrap();
        let successor_snapshot = serving.status().await.snapshot;
        assert!(serving
            .record_profile_unavailable_for_owned_generation(
                &id,
                receipt.generation,
                &fixture.owner
            )
            .await
            .unwrap()
            .is_none());
        assert_eq!(serving.status().await.snapshot, successor_snapshot);
        assert!(serving
            .record_loaded_model_for_owned_profile(status, &fixture.owner, &receipt)
            .await
            .is_err());
        assert_eq!(serving.status().await.snapshot, successor_snapshot);
        fixture.owner.stop(&id).await.unwrap();
    }
    fn router_listener_launch(
        fixture: &Fixture,
        address: std::net::SocketAddr,
    ) -> (
        BinaryLaunchConfig,
        RuntimeProfileLaunchSpec,
        RuntimeProfileOperationGuard,
    ) {
        let (config, mut spec, guard) = listener_launch(fixture, address);
        spec.provider_mode = RuntimeProviderMode::LlamaCppRouter;
        spec.launch_strategy = RuntimeProfileLaunchStrategy::BinaryProcess(
            RuntimeProfileBinaryLaunchKind::LlamaCppRouter,
        );
        std::fs::write(spec.runtime_dir.join("models-preset.ini"), b"version = 1\n[*]\nload-on-startup = false\n[model/a]\nmodel = /a\n[model/b]\nctx-size = 4096\n").unwrap();
        (config, spec, guard)
    }

    fn vacant_address() -> std::net::SocketAddr {
        std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
    }

    #[tokio::test]
    async fn router_model_operation_serializes_context_and_retains_immutable_receipt() {
        let fixture = Fixture::new();
        let (config, spec, guard) = router_listener_launch(&fixture, vacant_address());
        let id = spec.profile_id.clone();
        let receipt = fixture
            .owner
            .launch(config, spec, None, None, guard)
            .await
            .unwrap()
            .observation
            .unwrap();
        wait_for_listener(&receipt).await;
        let operation = fixture
            .owner
            .begin_router_model_operation(&id, &receipt)
            .unwrap();
        assert!(fixture
            .owner
            .begin_router_model_operation(&id, &receipt)
            .is_err());
        drop(operation);
        let mut operation = fixture
            .owner
            .begin_router_model_operation(&id, &receipt)
            .unwrap();
        assert!(operation.set_model_context("model/a", 18000).await.unwrap());
        assert!(!operation.set_model_context("model/a", 18000).await.unwrap());
        assert_eq!(fixture.owner.snapshot(&id).unwrap().unwrap(), receipt);
        assert_eq!(std::fs::read_to_string(fixture.root.path().join("models-preset.ini")).unwrap(), "version = 1\n[*]\nload-on-startup = false\n[model/a]\nctx-size = 18000\nmodel = /a\n[model/b]\nctx-size = 4096\n");
        operation.finish().unwrap();
        let operation = fixture
            .owner
            .begin_router_model_operation(&id, &receipt)
            .unwrap();
        // Stop bypasses the model lease and invalidates its completion proof.
        fixture.owner.stop(&id).await.unwrap();
        assert!(operation.finish().is_err());
        let (config, spec, guard) = router_listener_launch(&fixture, vacant_address());
        let successor = fixture
            .owner
            .launch(config, spec, None, None, guard)
            .await
            .unwrap()
            .observation
            .unwrap();
        wait_for_listener(&successor).await;
        assert_ne!(receipt.generation, successor.generation);
        assert!(fixture
            .owner
            .begin_router_model_operation(&id, &receipt)
            .is_err());
        fixture
            .owner
            .begin_router_model_operation(&id, &successor)
            .unwrap()
            .finish()
            .unwrap();
        fixture.owner.stop(&id).await.unwrap();
    }

    #[tokio::test]
    async fn router_model_operation_external_edit_leaves_uncertain_until_stop() {
        let fixture = Fixture::new();
        let (config, spec, guard) = router_listener_launch(&fixture, vacant_address());
        let id = spec.profile_id.clone();
        let receipt = fixture
            .owner
            .launch(config, spec, None, None, guard)
            .await
            .unwrap()
            .observation
            .unwrap();
        wait_for_listener(&receipt).await;
        let mut operation = fixture
            .owner
            .begin_router_model_operation(&id, &receipt)
            .unwrap();
        let path = fixture.root.path().join("models-preset.ini");
        let edited = b"[model/a]\nctx-size = 17\n";
        std::fs::write(&path, edited).unwrap();
        assert!(operation.set_model_context("model/a", 18000).await.is_err());
        drop(operation);
        assert!(fixture
            .owner
            .begin_router_model_operation(&id, &receipt)
            .is_err());
        assert_eq!(std::fs::read(&path).unwrap(), edited);
        fixture.owner.stop(&id).await.unwrap();
        let (config, spec, guard) = router_listener_launch(&fixture, vacant_address());
        let successor = fixture
            .owner
            .launch(config, spec, None, None, guard)
            .await
            .unwrap()
            .observation
            .unwrap();
        wait_for_listener(&successor).await;
        fixture
            .owner
            .begin_router_model_operation(&id, &successor)
            .unwrap()
            .finish()
            .unwrap();
        fixture.owner.stop(&id).await.unwrap();
    }

    #[tokio::test]
    async fn router_model_operation_cancelled_method_cannot_release_pending_command() {
        use std::future::Future;
        let fixture = Fixture::new();
        let (config, spec, guard) = router_listener_launch(&fixture, vacant_address());
        let id = spec.profile_id.clone();
        let receipt = fixture
            .owner
            .launch(config, spec, None, None, guard)
            .await
            .unwrap()
            .observation
            .unwrap();
        wait_for_listener(&receipt).await;
        let (release_command, command_gate) = std::sync::mpsc::channel();
        {
            let registry = fixture.owner.registry.lock().unwrap();
            RouterModelState::set_command_gate(
                registry.sessions[&id].router_models.as_ref().unwrap(),
                command_gate,
            );
        }
        let mut operation = fixture
            .owner
            .begin_router_model_operation(&id, &receipt)
            .unwrap();
        let mut future = Box::pin(operation.set_model_context("model/a", 18000));
        let mut context = std::task::Context::from_waker(std::task::Waker::noop());
        assert!(future.as_mut().poll(&mut context).is_pending());
        drop(future);
        assert!(operation.set_model_context("model/a", 4096).await.is_err());
        assert!(operation.finish().is_err());
        assert!(fixture
            .owner
            .begin_router_model_operation(&id, &receipt)
            .is_err());
        let _ = release_command.send(());
        fixture.owner.stop(&id).await.unwrap();
        assert!(!fixture.root.path().join("runtime.pid").exists());
    }
}
