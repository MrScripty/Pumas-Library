//! Pumas RPC Server - JSON-RPC backend for Electron IPC.
//!
//! This binary provides a JSON-RPC 2.0 server that wraps the pumas-core library
//! for communication with the Electron main process.

mod catalog_projection;
mod contract;
mod discovery;
mod handlers;
mod http_admission;
mod http_transport;
mod owner_retention;
#[cfg(feature = "inference-plugins")]
mod provider_clients;
#[cfg(feature = "s3")]
mod s3_imports;
mod server;
mod startup;
mod wrapper;

use anyhow::Result;
use clap::Parser;
#[cfg(feature = "inference-plugins")]
use pumas_library::AppId;
use std::io::Write;
use std::path::PathBuf;
use tokio::runtime::Builder;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

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
    /// Port to listen on (0 = auto-assign)
    #[arg(short, long, default_value = "0")]
    port: u16,

    /// Host to bind to
    #[arg(long, default_value = "127.0.0.1")]
    host: String,

    /// Grace for accepted HTTP connections after shutdown; not a request timeout
    #[arg(long, default_value_t = http_transport::DEFAULT_HTTP_SHUTDOWN_GRACE_MS)]
    http_shutdown_grace_ms: u64,

    /// Enable debug logging
    #[arg(short, long)]
    debug: bool,

    /// Launcher root directory (defaults to current directory's parent)
    #[arg(long)]
    launcher_root: Option<PathBuf>,
}

fn main() -> Result<()> {
    let args = Args::parse();
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
    let log_level = if args.debug {
        Level::DEBUG
    } else {
        Level::INFO
    };
    FmtSubscriber::builder()
        .with_max_level(log_level)
        .with_target(false)
        .with_thread_ids(false)
        .compact()
        .init();

    let runtime = Builder::new_multi_thread()
        .enable_all()
        .worker_threads(RPC_WORKER_THREADS)
        .max_blocking_threads(RPC_MAX_BLOCKING_THREADS)
        .thread_name("pumas-rpc")
        .build()?;

    runtime.block_on(run(args, host, http_policy))
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
    let signal = async move {
        #[cfg(windows)]
        {
            tokio::select! {
                Some(()) = interrupt.recv() => {},
                Some(()) = ctrl_break.recv() => {}
            }
        }
        #[cfg(unix)]
        tokio::select! {
            _ = interrupt.recv() => {},
            _ = terminate.recv() => {},
        }
        #[cfg(not(any(unix, windows)))]
        tokio::signal::ctrl_c().await?;
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
