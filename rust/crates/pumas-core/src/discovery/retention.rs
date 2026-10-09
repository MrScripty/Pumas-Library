//! Passive current-generation retention through the existing custody coordinator.
use super::InstanceDescription;
use crate::{PumasApi, PumasError, Result};
use tokio::sync::oneshot;

/// Opaque, non-cloneable, non-serializable passive retention of this generation.
/// It grants no effect, startup, shutdown or historical recovery authority.
/// Release/drop ends only this passive hold; independently admitted work retains
/// its own custody. Shutdown can close admission while waiting for this guard.
pub struct LocalOwnerRetention {
    description: InstanceDescription,
    release: Option<oneshot::Sender<()>>,
    observed: Option<oneshot::Receiver<Result<()>>>,
}

impl LocalOwnerRetention {
    pub fn description(&self) -> &InstanceDescription {
        &self.description
    }

    /// End the passive hold and observe its existing custody task result.
    /// This is not an owner/effect cessation receipt or permission to reclaim.
    pub async fn release(mut self) -> Result<()> {
        self.end_hold();
        self.observed.take().unwrap().await.map_err(|_| {
            invalid("local retention observer unavailable; owner cessation is unconfirmed")
        })?
    }

    fn end_hold(&mut self) {
        if let Some(release) = self.release.take() {
            let _ = release.send(());
        }
    }
}

impl Drop for LocalOwnerRetention {
    fn drop(&mut self) {
        // A passive handle ending is known; no external writer is inferred to
        // have stopped. Loss of the task executor still retains unresolved rows.
        self.end_hold();
    }
}

impl PumasApi {
    /// Retain this exact ready generation using existing physical lifetime and
    /// external-service task custody. No lock, owner table or new claim is made.
    /// Ordinary API drop/shutdown may close availability, but exact row release
    /// waits for all admitted passive guards and the existing owner drains.
    pub fn retain_local_owner(
        &self,
        expected: &InstanceDescription,
    ) -> Result<LocalOwnerRetention> {
        if !cfg!(target_os = "linux") {
            return Err(invalid("local owner retention is qualified only on Linux"));
        }
        let description = self.instance_description()?;
        if description != *expected {
            return Err(invalid("local retention owner identity changed"));
        }
        let primary = self.primary();
        primary
            .external_service_tasks
            .require_store_root(&description.library_root)?;
        let (release, released) = oneshot::channel();
        // start_owned's synchronous close gate linearizes admission with begin().
        // Its actual task retains StoreLifetime independently of this observer.
        let observed = primary.external_service_tasks.start_owned(
            "local-owner-passive-retention",
            move |_| async move {
                let _ = released.await;
                Ok(())
            },
        )?;
        let retention = LocalOwnerRetention {
            description,
            release: Some(release),
            observed: Some(observed),
        };
        if self.instance_description()? != *retention.description() {
            return Err(invalid("local retention owner changed during admission"));
        }
        Ok(retention)
    }
}

fn invalid(message: &str) -> PumasError {
    PumasError::InvalidParams {
        message: message.into(),
    }
}
