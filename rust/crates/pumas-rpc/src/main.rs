//! Pumas RPC Server - JSON-RPC backend for Electron IPC.
//!
//! This binary provides a JSON-RPC 2.0 server that wraps the pumas-core library
//! for communication with the Electron main process.

mod catalog_projection;
mod contract;
mod discovery;
mod handlers;
mod http_admission;
mod http_rejection;
mod http_transport;
mod launch;
mod local_enumeration;
mod owner_retention;
#[cfg(feature = "inference-plugins")]
mod provider_clients;
mod public_text;
#[cfg(feature = "s3")]
mod s3_imports;
mod server;
mod startup;
mod startup_failure;
mod wrapper;

use anyhow::Result;
use clap::Parser;
#[cfg(feature = "inference-plugins")]
use pumas_library::AppId;
use std::io::Write;
use std::path::PathBuf;
use tokio::runtime::Builder;
use tracing::info;
use tracing_subscriber::{EnvFilter, FmtSubscriber};

const RPC_WORKER_THREADS: usize = 4;
const RPC_MAX_BLOCKING_THREADS: usize = 16;
#[cfg(feature = "inference-plugins")]
const VERSION_MANAGED_APPS: &[AppId] = &[AppId::Ollama, AppId::Torch, AppId::LlamaCpp];

async fn finish_signal_observer(task: &mut tokio::task::JoinHandle<Result<()>>) -> Result<()> {
    if !task.is_finished() {
        task.abort();
    }
    match task.await {
        Ok(result) => result,
        Err(error) if error.is_cancelled() => Ok(()), // Only passive signal observation.
        Err(error) => Err(error.into()),
    }
}

fn observe_both(first: Result<()>, signal: Result<()>) -> Result<()> {
    match (first, signal) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
        (Err(first), Err(signal)) => Err(anyhow::anyhow!(
            "{first}; signal observation failed: {signal}"
        )),
    }
}

#[derive(Parser, Debug)]
#[command(name = "pumas-rpc")]
#[command(about = "JSON-RPC server for Pumas Library")]
struct Args {
    /// Back up and explicitly migrate a stopped library's download store to schema 7.
    #[arg(long, requires_all = ["launcher_root", "confirm_old_writers_stopped"], conflicts_with_all = ["build_info", "discover_local", "describe_local_http", "attach_or_start_local_http", "retain_local_http_owner"])]
    #[cfg_attr(feature = "export-contract", arg(conflicts_with_all = ["export_desktop_contract", "export_desktop_fixtures"]))]
    migrate_download_store_offline: bool,
    /// Confirm every old reader/writer is stopped; schema 7 has no old-binary rollback.
    #[arg(long, requires = "migrate_download_store_offline")]
    confirm_old_writers_stopped: bool,
    /// List unverified registered library roots without starting or contacting owners.
    #[arg(long, conflicts_with_all = ["build_info", "describe_local_http", "attach_or_start_local_http", "retain_local_http_owner", "launcher_root"])]
    #[cfg_attr(feature = "export-contract", arg(conflicts_with_all = ["export_desktop_contract", "export_desktop_fixtures"]))]
    discover_local: bool,
    /// Print build/protocol/schema identity without starting a runtime or server.
    #[arg(long)]
    build_info: bool,
    /// Authenticate and describe an existing selected local HTTP owner; never start one.
    #[arg(long, requires = "launcher_root", conflicts_with = "build_info")]
    #[cfg_attr(feature = "export-contract", arg(conflicts_with_all = ["export_desktop_contract", "export_desktop_fixtures"]))]
    describe_local_http: bool,
    /// Explicit selected-root borrow or reserved control-plane start (Linux).
    #[arg(long, requires = "launcher_root", conflicts_with_all = ["build_info", "describe_local_http"])]
    #[cfg_attr(feature = "export-contract", arg(conflicts_with_all = ["export_desktop_contract", "export_desktop_fixtures"]))]
    attach_or_start_local_http: bool,
    /// Hold passive retention of an authenticated existing HTTP owner (Linux).
    #[arg(long, requires = "launcher_root", conflicts_with_all = ["build_info", "describe_local_http", "attach_or_start_local_http"])]
    #[cfg_attr(feature = "export-contract", arg(conflicts_with_all = ["export_desktop_contract", "export_desktop_fixtures"]))]
    retain_local_http_owner: bool,
    /// Export the current desktop wire contract without starting a server.
    #[cfg(feature = "export-contract")]
    #[arg(long)]
    export_desktop_contract: bool,
    /// Produce real constructor-generated conformance fixtures in a temporary library.
    #[cfg(feature = "export-contract")]
    #[arg(long)]
    export_desktop_fixtures: bool,
    /// Release the library's instance entry left by a Pumas process that is no
    /// longer running, after an operator confirmed it. Needs the entry's PID and
    /// start time as reported in the `stale-instance` startup failure.
    #[arg(long, requires_all = ["launcher_root", "expected_pid", "expected_started_at"], conflicts_with_all = ["build_info", "discover_local", "describe_local_http", "attach_or_start_local_http", "retain_local_http_owner", "migrate_download_store_offline"])]
    release_stale_instance: bool,
    /// The confirmed entry's PID (with --release-stale-instance).
    #[arg(long, requires = "release_stale_instance")]
    expected_pid: Option<u32>,
    /// The confirmed entry's start time (with --release-stale-instance).
    #[arg(long, requires = "release_stale_instance")]
    expected_started_at: Option<String>,
    /// Port to listen on (0 = auto-assign)
    #[arg(short, long, default_value = "0")]
    port: u16,

    /// Host to bind to
    #[arg(long, default_value = "127.0.0.1")]
    host: String,

    /// Grace for accepted HTTP connections after shutdown; not a request timeout
    #[arg(long, default_value_t = http_transport::DEFAULT_HTTP_SHUTDOWN_GRACE_MS)]
    http_shutdown_grace_ms: u64,

    /// Shut down through the normal drain when stdin reaches end of file. The
    /// desktop holds the write end, so its exit (including a crash) stops this
    /// process. Other launchers do not pass it and keep their stdin semantics.
    #[arg(long)]
    exit_on_stdin_eof: bool,

    /// Enable debug logging for the Pumas crates (RUST_LOG, when set, takes precedence)
    #[arg(short, long)]
    debug: bool,

    /// Launcher root directory (defaults to current directory's parent)
    #[arg(long)]
    launcher_root: Option<PathBuf>,
}

fn main() -> Result<()> {
    // Before any thread or child process can observe the environment.
    launch::take_from_env()?;
    let args = Args::parse();
    if args.migrate_download_store_offline {
        let backup = pumas_library::model_library::DownloadPersistence::migrate_library_offline(
            args.launcher_root
                .as_deref()
                .expect("clap requires launcher root"),
        )?;
        println!(
            "Download database upgraded to schema 7. Original backup: {}",
            backup.display()
        );
        return Ok(());
    }
    if args.discover_local {
        return local_enumeration::write();
    }
    if args.release_stale_instance {
        return release_stale_instance(
            args.launcher_root
                .as_deref()
                .expect("clap requires launcher root"),
            args.expected_pid.expect("clap requires expected pid"),
            args.expected_started_at
                .as_deref()
                .expect("clap requires expected start"),
        );
    }
    if args.retain_local_http_owner {
        let runtime = Builder::new_current_thread().enable_all().build()?;
        return runtime.block_on(owner_retention::retain_from_cli(
            args.launcher_root
                .as_deref()
                .expect("clap requires launcher root"),
        ));
    }
    if args.build_info {
        serde_json::to_writer_pretty(std::io::stdout(), &discovery::build_info())?;
        return Ok(());
    }
    #[cfg(feature = "export-contract")]
    if args.export_desktop_fixtures {
        serde_json::to_writer_pretty(std::io::stdout(), &contract::desktop_contract_fixtures()?)?;
        return Ok(());
    }
    #[cfg(feature = "export-contract")]
    if args.export_desktop_contract {
        serde_json::to_writer_pretty(std::io::stdout(), &contract::desktop_contract_schema()?)?;
        return Ok(());
    }
    if args.describe_local_http {
        let runtime = Builder::new_current_thread().enable_all().build()?;
        let root = args
            .launcher_root
            .as_deref()
            .expect("clap requires launcher root");
        let description = runtime.block_on(discovery::describe_local_http(root))?;
        serde_json::to_writer_pretty(std::io::stdout(), &description)?;
        return Ok(());
    }
    if args.attach_or_start_local_http && !cfg!(target_os = "linux") {
        anyhow::bail!("local HTTP bootstrap is qualified only for Linux builds");
    }

    let host = server::LoopbackHost::parse(&args.host)?;
    let http_policy = http_transport::HttpShutdownPolicy::from_millis(args.http_shutdown_grace_ms)?;

    // Set up logging
    let (filter, rejected_rust_log) = log_filter(args.debug, std::env::var("RUST_LOG").ok());
    FmtSubscriber::builder()
        .with_env_filter(filter)
        .with_target(false)
        .with_thread_ids(false)
        .compact()
        .init();
    if let Some(reason) = rejected_rust_log {
        tracing::warn!(%reason, "RUST_LOG is invalid; using the default log filter");
    }

    let runtime = Builder::new_multi_thread()
        .enable_all()
        .worker_threads(RPC_WORKER_THREADS)
        .max_blocking_threads(RPC_MAX_BLOCKING_THREADS)
        .thread_name("pumas-rpc")
        .build()?;

    runtime.block_on(run(args, host, http_policy))
}

/// Release exactly the confirmed stale entry, re-checking it first.
fn release_stale_instance(root: &std::path::Path, pid: u32, started_at: &str) -> Result<()> {
    FmtSubscriber::builder()
        .with_env_filter(EnvFilter::new(DEFAULT_LOG_FILTER))
        .with_target(false)
        .compact()
        .with_writer(std::io::stderr)
        .init();
    let root = pumas_library::platform::paths::absolute_launcher_root(root)?;
    let registry = pumas_library::registry::LibraryRegistry::open()?;
    let stale = registry.inspect_stale_instance(&root)?.ok_or_else(|| {
        anyhow::anyhow!(
            "The library has no releasable instance entry: it is not registered, its process is running, or a running Pumas still holds it"
        )
    })?;
    if stale.pid != pid || stale.started_at != started_at {
        anyhow::bail!(
            "The library's instance entry changed since it was confirmed (now pid {}, started {}); check it again",
            stale.pid,
            stale.started_at
        );
    }
    registry.release_stale_instance(&root, &stale)?;
    println!(
        "{}",
        serde_json::json!({ "released": true, "pid": stale.pid })
    );
    Ok(())
}

/// Resolves when stdin reaches end of file or fails; never when `enabled` is
/// false. Reads happen on a dedicated thread so a parent that keeps the pipe
/// open never occupies the runtime's blocking pool.
async fn stdin_closed(enabled: bool) {
    if !enabled {
        return std::future::pending().await;
    }
    let (closed, receiver) = tokio::sync::oneshot::channel::<()>();
    let spawned = std::thread::Builder::new()
        .name("pumas-rpc-stdin".into())
        .spawn(move || {
            use std::io::Read;
            let mut stdin = std::io::stdin().lock();
            let mut discard = [0u8; 1024];
            loop {
                match stdin.read(&mut discard) {
                    Ok(0) => break,
                    Ok(_) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
                    Err(_) => break,
                }
            }
            let _ = closed.send(());
        });
    if let Err(error) = spawned {
        tracing::warn!(%error, "Could not watch stdin; --exit-on-stdin-eof is inactive");
        return std::future::pending().await;
    }
    let _ = receiver.await;
}

/// `--debug` raises only the Pumas crates to DEBUG, so dependencies stay
/// quiet; a valid `RUST_LOG` replaces this default entirely.
const DEFAULT_LOG_FILTER: &str = "info";
const DEBUG_LOG_FILTER: &str = "info,pumas_rpc=debug,pumas_library=debug,pumas_app_manager=debug";

/// The log filter, and why `RUST_LOG` was ignored if it was set but invalid.
fn log_filter(debug: bool, rust_log: Option<String>) -> (EnvFilter, Option<String>) {
    let default = || {
        EnvFilter::new(if debug {
            DEBUG_LOG_FILTER
        } else {
            DEFAULT_LOG_FILTER
        })
    };
    match rust_log.filter(|value| !value.trim().is_empty()) {
        Some(value) => match EnvFilter::try_new(&value) {
            Ok(filter) => (filter, None),
            Err(error) => (default(), Some(error.to_string())),
        },
        None => (default(), None),
    }
}

async fn run(
    mut args: Args,
    host: server::LoopbackHost,
    http_policy: http_transport::HttpShutdownPolicy,
) -> Result<()> {
    // Install Unix handlers before readiness is published. Electron and service
    // managers use SIGTERM, which must enter the same owned drain as SIGINT.
    #[cfg(unix)]
    let mut terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    #[cfg(unix)]
    let mut interrupt = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())?;

    #[cfg(windows)]
    let mut interrupt = tokio::signal::windows::ctrl_c()?;
    #[cfg(windows)]
    let mut ctrl_break = tokio::signal::windows::ctrl_break()?;

    info!("Starting Pumas RPC Server");

    // Determine launcher root
    let launcher_root = match args.launcher_root.take() {
        Some(path) => path,
        None => {
            // Default: assume we're in rust/target/*/pumas-rpc, go up to find project root
            let exe_path = std::env::current_exe()?;
            let mut path = exe_path.parent().unwrap().to_path_buf();

            // Navigate up from target directory to find project root
            while path.file_name().map(|n| n != "rust").unwrap_or(false) {
                if let Some(parent) = path.parent() {
                    path = parent.to_path_buf();
                } else {
                    break;
                }
            }

            // Go up one more level from rust/ to project root
            path.parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| std::env::current_dir().unwrap())
        }
    };

    let launcher_root = pumas_library::platform::paths::absolute_launcher_root(&launcher_root)?;

    info!("Launcher root configured");

    // Both OS signals and RPC admission converge on the same owned receipt.
    // With --exit-on-stdin-eof, the parent closing stdin is one more such signal.
    let parent_gone = stdin_closed(args.exit_on_stdin_eof);
    let signal = async move {
        #[cfg(windows)]
        {
            tokio::select! {
                Some(()) = interrupt.recv() => {},
                Some(()) = ctrl_break.recv() => {}
                () = parent_gone => info!("stdin closed; shutting down"),
            }
        }
        #[cfg(unix)]
        tokio::select! {
            _ = interrupt.recv() => {},
            _ = terminate.recv() => {},
            () = parent_gone => info!("stdin closed; shutting down"),
        }
        #[cfg(not(any(unix, windows)))]
        tokio::select! {
            result = tokio::signal::ctrl_c() => result?,
            () = parent_gone => info!("stdin closed; shutting down"),
        }
        Ok::<(), anyhow::Error>(())
    };
    let stop = server::ShutdownRequest::default();
    let signal_stop = stop.clone();
    // Passive signal observation continues while actual constructors/readiness
    // are awaited. A signal never aborts the independently owned creator.
    let mut signal_task = tokio::spawn(async move {
        let result = signal.await;
        signal_stop.request();
        result
    });
    let explicit_bootstrap = args.attach_or_start_local_http;
    let task = tokio::spawn(startup::start(
        args,
        launcher_root.clone(),
        host,
        http_policy,
        stop.clone(),
    ));
    let started = match task.await {
        Ok(Ok(started)) => started,
        result => {
            let signal_result = finish_signal_observer(&mut signal_task).await;
            let error = match result {
                Ok(Err(error)) => error,
                Err(error) => {
                    anyhow::anyhow!("Startup supervisor failed; custody unconfirmed: {error}")
                }
                _ => unreachable!(),
            };
            startup_failure::write_for_root(&error, &launcher_root, &mut std::io::stdout().lock())?;
            return Err(match signal_result {
                Ok(()) => error,
                Err(signal) => {
                    anyhow::anyhow!("Startup failed: {error}; signal observation failed: {signal}")
                }
            });
        }
    };
    let (server, selected_root) = match started {
        startup::Started::Cancelled => {
            return finish_signal_observer(&mut signal_task).await;
        }
        startup::Started::Borrowed(description) => {
            let result = if stop.is_requested() {
                Ok(())
            } else {
                discovery::print_local_access("borrowed", &description)
            };
            let signal_result = finish_signal_observer(&mut signal_task).await;
            return observe_both(result, signal_result);
        }
        startup::Started::Owned { server, root } => (server, root),
    };
    let readiness = async {
        let description = if explicit_bootstrap {
            Some(discovery::describe_local_http(&selected_root).await?)
        } else {
            None
        };
        if stop.is_requested() {
            return Ok::<bool, anyhow::Error>(false);
        }
        // Fallible writes and flush are part of readiness observation; stdout
        // failure must still enter the observed owner drain.
        {
            let mut stdout = std::io::stdout().lock();
            writeln!(stdout, "RPC_PORT={}", server.addr().port())?;
            writeln!(
                stdout,
                "{}",
                launch::ready_line(server.addr().port(), launch::nonce())
            )?;
            stdout.flush()?;
        }
        if let Some(description) = description {
            if stop.is_requested() {
                return Ok(false);
            }
            discovery::print_local_access("owned", &description)?;
        }
        Ok(true)
    }
    .await;
    if !matches!(readiness, Ok(true)) {
        let drain = server.shutdown().await;
        let signal_result = finish_signal_observer(&mut signal_task).await;
        let drain = observe_both(drain, signal_result);
        return match (readiness, drain) {
            (Ok(_), Ok(())) => Ok(()),
            (Err(error), Ok(())) | (Ok(_), Err(error)) => Err(error),
            (Err(output), Err(drain)) => Err(anyhow::anyhow!(
                "Readiness failed: {output}; shutdown failed: {drain}"
            )),
        };
    }
    info!("RPC server running on {}", server.addr());
    tokio::select! {
        result = &mut signal_task => {
            let signal_result = result.map_err(anyhow::Error::from).and_then(|result| result);
            let drain = server.shutdown().await;
            match (signal_result, drain) {
                (Ok(()), Ok(())) => {},
                (Err(error), Ok(())) | (Ok(()), Err(error)) => return Err(error),
                (Err(signal), Err(drain)) => return Err(anyhow::anyhow!("Signal observation failed: {signal}; shutdown failed: {drain}")),
            }
        }
        result = server.wait() => {
            let signal_result = finish_signal_observer(&mut signal_task).await;
            observe_both(result, signal_result)?;
        },
    }
    info!("RPC shutdown completed");
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn rust_log_selects_the_filter_and_debug_raises_only_pumas_crates() {
        let (filter, rejected) = super::log_filter(false, None);
        assert_eq!(filter.to_string(), "info");
        assert!(rejected.is_none());
        let (filter, _) = super::log_filter(true, None);
        assert!(filter.to_string().contains("pumas_rpc=debug"));
        assert!(!filter.to_string().starts_with("debug"));
        let (filter, rejected) = super::log_filter(true, Some("warn,pumas_rpc=trace".into()));
        let rendered = filter.to_string();
        assert!(
            rendered.contains("pumas_rpc=trace") && rendered.contains("warn"),
            "{rendered}"
        );
        assert!(rejected.is_none());
        let (filter, rejected) = super::log_filter(false, Some("pumas_rpc=[".into()));
        assert_eq!(filter.to_string(), "info");
        assert!(rejected.is_some());
    }

    #[cfg(feature = "inference-plugins")]
    use super::VERSION_MANAGED_APPS;
    #[cfg(feature = "inference-plugins")]
    use pumas_library::AppId;

    #[cfg(feature = "inference-plugins")]
    #[test]
    fn version_managed_apps_are_the_installable_inference_runtimes() {
        assert_eq!(
            VERSION_MANAGED_APPS,
            &[AppId::Ollama, AppId::Torch, AppId::LlamaCpp]
        );
        assert!(!VERSION_MANAGED_APPS.contains(&AppId::OnnxRuntime));
        assert!(VERSION_MANAGED_APPS.iter().all(AppId::has_version_manager));
    }
}

#[cfg(test)]
mod discovery_cli_tests {
    use super::*;

    #[test]
    fn offline_migration_requires_explicit_root_and_historical_writer_confirmation() {
        let complete = [
            "pumas-rpc",
            "--migrate-download-store-offline",
            "--launcher-root",
            "/selected",
            "--confirm-old-writers-stopped",
        ];
        assert!(
            Args::try_parse_from(complete)
                .unwrap()
                .migrate_download_store_offline
        );
        assert!(Args::try_parse_from(&complete[..4]).is_err());
        assert!(Args::try_parse_from([
            "pumas-rpc",
            "--migrate-download-store-offline",
            "--confirm-old-writers-stopped"
        ])
        .is_err());
        assert!(Args::try_parse_from(["pumas-rpc", "--confirm-old-writers-stopped"]).is_err());
        for mode in [
            "--build-info",
            "--discover-local",
            "--describe-local-http",
            "--attach-or-start-local-http",
            "--retain-local-http-owner",
        ] {
            assert!(Args::try_parse_from(complete.into_iter().chain([mode])).is_err());
        }
    }

    #[test]
    fn observation_requires_explicit_root_and_distinct_mode() {
        assert!(Args::try_parse_from(["pumas-rpc", "--describe-local-http"]).is_err());
        assert!(Args::try_parse_from([
            "pumas-rpc",
            "--describe-local-http",
            "--launcher-root",
            "/selected",
            "--build-info"
        ])
        .is_err());
        let args = Args::try_parse_from([
            "pumas-rpc",
            "--describe-local-http",
            "--launcher-root",
            "/selected",
        ])
        .unwrap();
        assert!(args.describe_local_http);
        assert_eq!(args.launcher_root, Some(PathBuf::from("/selected")));
    }
}
