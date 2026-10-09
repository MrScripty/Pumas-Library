//! Process-owned constructor sequencing into the existing generic HTTP owner.
use crate::{discovery, http_transport::HttpShutdownPolicy, server, Args};
use anyhow::Result;
use pumas_library::{
    discovery::{
        prepare_local_access, CompatibilityRequirements, HttpServiceDescription, LocalAccess,
        LocalStartupCustody, PreparedLocalAccess,
    },
    PumasApi,
};
use std::{path::PathBuf, sync::Arc};
#[cfg(feature = "inference-plugins")]
use {
    pumas_app_manager::{SizeCalculator, VersionManager},
    pumas_library::{AppId, PluginLoader},
    std::collections::HashMap,
    tracing::warn,
};

pub(crate) enum Started {
    Borrowed(Box<HttpServiceDescription>),
    Owned {
        server: server::ServerHandle,
        root: PathBuf,
    },
    Cancelled,
}

async fn failed_initialization(
    api: &PumasApi,
    #[cfg(feature = "inference-plugins")] managers: &HashMap<String, VersionManager>,
    custody: Option<LocalStartupCustody>,
    error: anyhow::Error,
) -> anyhow::Error {
    let mut failures = vec![format!("initialization: {error:#}")];
    if let Some(custody) = custody {
        if let Err(error) = custody.complete(Err(pumas_library::PumasError::Other(
            "local initializer failed before supervisor handoff".into(),
        ))) {
            failures.push(error.to_string());
        }
    }
    #[cfg(feature = "inference-plugins")]
    for result in futures::future::join_all(
        managers
            .values()
            .map(VersionManager::shutdown_installations),
    )
    .await
    {
        if let Err(error) = result {
            failures.push(format!("installation drain: {error}"));
        }
    }
    if let Err(error) = api.shutdown_instance().await {
        failures.push(format!("core cessation: {error}"));
    }
    anyhow::anyhow!(failures.join("; "))
}

/// The caller spawns and observes this task without aborting it on a stop signal.
/// No inference runtime or model is selected/loaded by this control-plane start.
pub(crate) async fn start(
    args: Args,
    mut root: PathBuf,
    host: server::LoopbackHost,
    policy: HttpShutdownPolicy,
    stop: server::ShutdownRequest,
) -> Result<Started> {
    if stop.is_requested() {
        return Ok(Started::Cancelled);
    }
    let api = if args.attach_or_start_local_http {
        // Refuse invalid roots before opening/mutating the selected registry.
        root = root.canonicalize()?;
        anyhow::ensure!(
            root.is_dir(),
            "local bootstrap requires an existing directory root"
        );
        let registry = pumas_library::registry::LibraryRegistry::open()?;
        match prepare_local_access(registry, &root, &CompatibilityRequirements::default()).await? {
            PreparedLocalAccess::Borrowed(_) => {
                return Ok(Started::Borrowed(Box::new(
                    discovery::describe_local_http(&root).await?,
                )));
            }
            PreparedLocalAccess::Start(authority) => {
                root = authority.library_root().to_path_buf();
                if stop.is_requested() {
                    authority.cancel()?;
                    return Ok(Started::Cancelled);
                }
                match authority.start().await? {
                    LocalAccess::Owned { api, .. } => api,
                    LocalAccess::Borrowed { .. } => {
                        unreachable!("opaque start creates its own generation")
                    }
                }
            }
        }
    } else {
        let api = PumasApi::builder(&root)
            .auto_create_dirs(true)
            .build()
            .await?;
        root = api.instance_description()?.library_root;
        api
    };
    // PumasApi is owning and its Drop initiates shutdown. Only Arc shares are
    // passive; the actual server state retains the one API through its drain.
    let api = Arc::new(api);
    let custody = if args.attach_or_start_local_http {
        Some(api.prepare_local_startup_custody()?)
    } else {
        None
    };
    #[cfg(feature = "inference-plugins")]
    let mut managers = HashMap::new();
    #[cfg(feature = "inference-plugins")]
    for app in crate::VERSION_MANAGED_APPS {
        let initialized = if *app == AppId::LlamaCpp {
            VersionManager::new_with_acquisition(&root, *app, api.acquisition().clone()).await
        } else {
            VersionManager::new(&root, *app).await
        };
        match initialized {
            Ok(manager) => {
                managers.insert(app.as_str().to_owned(), manager);
            }
            Err(error) if args.attach_or_start_local_http => {
                return Err(failed_initialization(&api, &managers, custody, error.into()).await);
            }
            Err(error) => warn!(app = %app, error = %error, "Version manager unavailable"),
        }
    }
    #[cfg(feature = "inference-plugins")]
    let sizes = SizeCalculator::new_with_cache(root.join("launcher-data/cache")).await;
    #[cfg(feature = "inference-plugins")]
    let plugins = match PluginLoader::new_async(root.join("launcher-data/plugins")).await {
        Ok(loader) => loader,
        Err(error) if args.attach_or_start_local_http => {
            return Err(failed_initialization(&api, &managers, custody, error.into()).await);
        }
        Err(_) => {
            warn!("Plugin loader initialization failed; using empty loader");
            match PluginLoader::new_async(std::env::temp_dir().join("pumas-plugins-fallback")).await
            {
                Ok(loader) => loader,
                Err(error) => {
                    return Err(failed_initialization(&api, &managers, custody, error.into()).await)
                }
            }
        }
    };
    // Keep observation shares for failures before the HTTP supervisor takes custody.
    let result = if args.attach_or_start_local_http {
        server::start_server_with_startup(
            api.clone(),
            #[cfg(feature = "inference-plugins")]
            managers.clone(),
            #[cfg(feature = "inference-plugins")]
            sizes,
            #[cfg(feature = "inference-plugins")]
            plugins,
            server::ServerStartup {
                host,
                port: args.port,
                http_policy: policy,
                custody,
                stop,
            },
        )
        .await
    } else {
        server::start_server(
            api.clone(),
            #[cfg(feature = "inference-plugins")]
            managers.clone(),
            #[cfg(feature = "inference-plugins")]
            sizes,
            #[cfg(feature = "inference-plugins")]
            plugins,
            host,
            args.port,
            policy,
        )
        .await
    };
    match result {
        Ok(server) => Ok(Started::Owned { server, root }),
        Err(error) => Err(failed_initialization(
            &api,
            #[cfg(feature = "inference-plugins")]
            &managers,
            None,
            error,
        )
        .await),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn returned_creator_keeps_one_real_owner_ready_for_authenticated_rpc() {
        // Other RPC fixtures temporarily mutate the global registry setting.
        // Exercise the actual startup/observation contract in one isolated
        // process, without changing production registry selection or authority.
        const CHILD: &str = "PUMAS_STARTUP_OWNER_PROOF_CHILD";
        if std::env::var_os(CHILD).is_none() {
            let fixture = tempfile::tempdir().unwrap();
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "startup::tests::returned_creator_keeps_one_real_owner_ready_for_authenticated_rpc",
                    "--nocapture",
                ])
                .env(CHILD, "1")
                .env("PUMAS_REGISTRY_DB_PATH", fixture.path().join("registry.db"))
                .env("XDG_CONFIG_HOME", fixture.path().join("config"))
                .env("XDG_CACHE_HOME", fixture.path().join("cache"))
                .output()
                .unwrap();
            print!("{}", String::from_utf8_lossy(&output.stdout));
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            return;
        }
        let fixture = tempfile::tempdir().unwrap();
        let root = fixture.path().to_path_buf();
        let args = Args::try_parse_from([
            "pumas-rpc",
            "--attach-or-start-local-http",
            "--launcher-root",
            root.to_str().unwrap(),
        ])
        .unwrap();
        let created = tokio::spawn(start(
            args,
            root.clone(),
            server::LoopbackHost::parse("127.0.0.1").unwrap(),
            HttpShutdownPolicy::default(),
            server::ShutdownRequest::default(),
        ))
        .await
        .unwrap()
        .unwrap();
        let Started::Owned { server, .. } = created else {
            panic!("expected actual owner");
        };
        let description = discovery::describe_local_http(&root).await.unwrap();
        let reply = reqwest::Client::builder()
            .no_proxy()
            .build()
            .unwrap()
            .post(format!("{}/rpc", description.endpoint.as_str()))
            .header(
                "Pumas-Instance-Generation",
                &description.instance.generation,
            )
            .header("Pumas-Service-Generation", &description.service_generation)
            .json(&serde_json::json!({"jsonrpc":"2.0","id":1,"method":"get_models"}))
            .send()
            .await
            .unwrap();
        assert_eq!(reply.status(), reqwest::StatusCode::OK);
        assert_eq!(
            reply.json::<serde_json::Value>().await.unwrap()["result"]["models"],
            serde_json::json!({})
        );
        server.shutdown().await.unwrap();
        server.shutdown().await.unwrap();
        assert!(discovery::describe_local_http(&root).await.is_err());
    }

    #[tokio::test]
    async fn already_latched_stop_refuses_before_registry_or_constructor_effects() {
        let fixture = tempfile::tempdir().unwrap();
        let root = fixture.path().join("uncreated");
        let args = Args::try_parse_from([
            "pumas-rpc",
            "--attach-or-start-local-http",
            "--launcher-root",
            root.to_str().unwrap(),
        ])
        .unwrap();
        let stop = server::ShutdownRequest::default();
        stop.request();
        let result = start(
            args,
            root.clone(),
            server::LoopbackHost::parse("127.0.0.1").unwrap(),
            HttpShutdownPolicy::default(),
            stop,
        )
        .await
        .unwrap();
        assert!(matches!(result, Started::Cancelled));
        assert!(!root.exists());
        assert_eq!(std::fs::read_dir(fixture.path()).unwrap().count(), 0);
    }
}
