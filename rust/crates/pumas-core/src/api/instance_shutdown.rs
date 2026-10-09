//! Ordered local owner cessation. A dropped waiter never abandons this coordinator.
use super::PrimaryState;
use crate::{PumasApi, PumasError, Result};
use futures::future::{BoxFuture, Shared};
use futures::FutureExt;
use std::sync::Arc;

pub(crate) type InstanceShutdownReceipt =
    Shared<BoxFuture<'static, std::result::Result<(), Arc<String>>>>;

pub(crate) fn begin(primary: &Arc<PrimaryState>) -> InstanceShutdownReceipt {
    primary.external_service_tasks.close();
    primary
        .instance_shutdown
        .get_or_init(|| {
            let state = primary.clone();
            let task = primary
                .runtime_tasks
                .runtime_handle()
                .spawn(async move { settle(state).await });
            async move {
                match task.await {
                    Ok(result) => result.map_err(|error| Arc::new(error.to_string())),
                    Err(error) => Err(Arc::new(format!(
                        "instance shutdown coordinator failed: {error}"
                    ))),
                }
            }
            .boxed()
            .shared()
        })
        .clone()
}

async fn settle(primary: Arc<PrimaryState>) -> Result<()> {
    primary.runtime_tasks.close();
    // Taking the handle breaks the state/accept-task cycle. Shutdown closes
    // transport admission; admitted IPC dispatches must still finish and join.
    let server = primary.server_handle.lock().await.take();
    let mut server = server;
    if let Some(server) = &mut server {
        server.shutdown();
    }
    let client = primary.hf_client.clone();
    let finite = primary
        .runtime_tasks
        .shutdown_owned_then(move || async move {
            match client {
                Some(client) => client.shutdown_downloads().await,
                None => Ok(()),
            }
        });
    let ipc = async {
        match server {
            Some(server) => server.shutdown_and_wait().await,
            None => Ok(()),
        }
    };
    // Every owner is observed even when another drain fails.
    let (finite, ipc, setup, conversions, runtimes, services) = tokio::join!(
        finite,
        ipc,
        primary.conversion_manager.shutdown_setup(),
        primary.conversion_manager.shutdown(),
        super::state_runtime_profiles::stop_all_managed_runtime_profiles(&primary),
        primary.external_service_tasks.shutdown_owned(),
    );
    let acquisition = primary.acquisition.shutdown().await;
    let mut failures = Vec::new();
    for (name, result) in [
        ("finite work", finite),
        ("external services", services),
        ("IPC", ipc),
        ("conversion setup", setup),
        ("conversions", conversions),
        ("acquisition", acquisition),
    ] {
        if let Err(error) = result {
            failures.push(format!("{name}: {error}"));
        }
    }
    match runtimes {
        Ok(summary) => failures.extend(summary.errors),
        Err(error) => failures.push(format!("managed runtimes: {error}")),
    }
    if !failures.is_empty() {
        // Cessation failure/unknown never licenses another owner.
        return Err(PumasError::Other(format!(
            "instance shutdown incomplete: {}",
            failures.join("; ")
        )));
    }
    if let (Some(registry), Some(instance)) = (&primary.registry, primary.ready_instance.get()) {
        let _ = registry.release_ready_instance(instance)?;
    }
    Ok(())
}

impl PumasApi {
    /// Close local transport admission and observe owned finite effects, IPC,
    /// conversions, managed runtime profiles and acquisition before releasing
    /// this exact registry generation. Cancellation leaves the shared coordinator
    /// running; failures retain ownership. On Linux/macOS physical exclusion
    /// continues until this API and all escaped lifetime-retaining handles/effects
    /// are dropped. This receipt does not authorize crash recovery.
    pub async fn shutdown_instance(&self) -> Result<()> {
        begin(self.primary())
            .await
            .map_err(|error| PumasError::Other((*error).clone()))
    }
}
