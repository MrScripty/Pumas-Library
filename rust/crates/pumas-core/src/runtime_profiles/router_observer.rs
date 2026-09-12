//! A retained, generation-owned SSE observer. The process session cancels and
//! joins this task. It holds a weak owner except while checking or mutating an
//! exact session, so no permanent owner/task cycle is possible.
use super::{
    generate_llama_cpp_router_catalog, OwnedRuntimeProfileObservation, RuntimeProfileLaunchSpec,
    RuntimeProfileProcessOwner,
};
use crate::model_library::ModelLibrary;
use crate::models::*;
use crate::serving::ServingService;
use crate::{PumasError, Result};
use futures::{FutureExt, StreamExt};
use std::sync::{Arc, Weak};
use std::time::Duration;
use tokio::sync::watch;

#[path = "router_catalog_sync.rs"]
mod catalog;
#[path = "router_snapshot.rs"]
mod snapshot;
const HTTP_BUDGET: Duration = Duration::from_secs(5);
const IDLE_LEASE: Duration = Duration::from_secs(30);
const RECONNECT: Duration = Duration::from_secs(1);

#[derive(Clone)]
pub(crate) struct RouterObserverContext {
    pub owner: Weak<RuntimeProfileProcessOwner>,
    pub library: Arc<ModelLibrary>,
    pub serving: Arc<ServingService>,
}
impl RouterObserverContext {
    pub(super) fn spawn(
        self,
        spec: RuntimeProfileLaunchSpec,
        generation: u64,
        mut stop: watch::Receiver<bool>,
        mut terminal: watch::Receiver<Option<bool>>,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut worker = Observer::new(self, spec, generation);
            let mut panic = None;
            if !*stop.borrow() {
                tokio::select! {
                    biased;
                    _ = stop.wait_for(|stopped| *stopped) => {},
                outcome = std::panic::AssertUnwindSafe(worker.run()).catch_unwind() => {panic = outcome.err();},
                }
            }
            worker.unavailable("Owned router observation stopped");
            let cleaned = terminal
                .wait_for(|value| value.is_some())
                .await
                .ok()
                .is_some_and(|value| *value == Some(true));
            if cleaned {
                if let Ok(owner) = worker.owner() {
                    let _ = worker
                        .context
                        .serving
                        .record_profile_unavailable_for_owned_generation(
                            &worker.spec.profile_id,
                            worker.generation,
                            &owner,
                        )
                        .await;
                }
            }
            if let Some(payload) = panic {
                std::panic::resume_unwind(payload);
            }
        })
    }
}
struct Observer {
    context: RouterObserverContext,
    spec: RuntimeProfileLaunchSpec,
    generation: u64,
    receipt: Option<OwnedRuntimeProfileObservation>,
    sync: RouterProfileSyncStatus,
    client: reqwest::Client,
    catalog_dirty: bool,
}
impl Observer {
    fn new(
        context: RouterObserverContext,
        spec: RuntimeProfileLaunchSpec,
        generation: u64,
    ) -> Self {
        let sync = RouterProfileSyncStatus {
            profile_id: spec.profile_id.clone(),
            generation,
            observation_state: RouterObservationState::Connecting,
            catalog_state: RouterCatalogState::Current,
            pending_model_ids: vec![],
            last_error: None,
        };
        Self {
            context,
            spec,
            generation,
            receipt: None,
            sync,
            client: reqwest::Client::builder()
                .connect_timeout(HTTP_BUDGET)
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .expect("router HTTP client"),
            catalog_dirty: true,
        }
    }
    fn owner(&self) -> Result<Arc<RuntimeProfileProcessOwner>> {
        self.context
            .owner
            .upgrade()
            .ok_or_else(|| error("Owned router API dropped"))
    }
    fn current(&mut self) -> Result<OwnedRuntimeProfileObservation> {
        let owner = self.owner()?;
        let receipt = owner
            .snapshot(&self.spec.profile_id)?
            .filter(|r| r.generation == self.generation)
            .ok_or_else(|| error("Owned router generation ended"))?;
        self.receipt = Some(receipt.clone());
        if !owner.owns_current_listener(&self.spec.profile_id, &receipt)? {
            return Err(error("Owned router listener unavailable"));
        }
        Ok(receipt)
    }
    fn unavailable(&mut self, message: &str) {
        self.sync.observation_state = RouterObservationState::Unavailable;
        self.sync.last_error = Some(message.into());
        if let (Some(receipt), Ok(owner)) = (&self.receipt, self.owner()) {
            let _ = self
                .context
                .serving
                .publish_router_sync(receipt, &owner, self.sync.clone());
        }
    }
    async fn run(&mut self) {
        let mut library_events = self.context.library.subscribe_model_library_update_events();
        let mut serving_events = self.context.serving.subscribe_updates();
        loop {
            let result = self
                .connected(&mut library_events, &mut serving_events)
                .await;
            self.unavailable(
                &result
                    .err()
                    .map(|e| e.to_string())
                    .unwrap_or_else(|| "Router SSE disconnected".into()),
            );
            if self.context.owner.strong_count() == 0 {
                return;
            }
            tokio::time::sleep(RECONNECT).await;
        }
    }
    async fn connected(
        &mut self,
        library_events: &mut tokio::sync::broadcast::Receiver<ModelLibraryUpdateEvent>,
        serving_events: &mut tokio::sync::broadcast::Receiver<ServingStatusUpdateFeed>,
    ) -> Result<()> {
        self.current()?;
        // Establish subscription before the snapshot to avoid losing a transition
        // between the initial read and event registration.
        let response = tokio::time::timeout(
            HTTP_BUDGET,
            self.client
                .get(format!(
                    "{}/models/sse",
                    self.spec.endpoint_url.as_str().trim_end_matches('/')
                ))
                .send(),
        )
        .await
        .map_err(|_| error("Router SSE connection timed out"))?
        .map_err(http_error)?
        .error_for_status()
        .map_err(http_error)?;
        if !response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .is_some_and(|v| v.starts_with("text/event-stream"))
        {
            return Err(error("Router SSE content type invalid"));
        }
        let mut stream = response.bytes_stream();
        self.sample().await?;
        let mut buffer = Vec::new();
        let mut lease = tokio::time::Instant::now() + IDLE_LEASE;
        loop {
            tokio::select! {
                _event=library_events.recv()=> {self.catalog_dirty=true;self.sample().await?;},
                event=serving_events.recv()=> {if event.is_err() {self.catalog_dirty=true;} self.sample().await?;},
                _ = tokio::time::sleep_until(lease) => {return Err(error("Router SSE observation lease expired"));},
                chunk=stream.next()=> {
                    let chunk=chunk.ok_or_else(||error("Router SSE disconnected"))?.map_err(http_error)?;
                    buffer.extend_from_slice(&chunk);
                    if buffer.len()>1024*1024 {return Err(error("Router SSE event exceeds size limit"));}
                    let mut changed=false;
                    while let Some(end)=buffer.windows(2).position(|bytes|bytes==b"\n\n") {
                        let frame=String::from_utf8(buffer.drain(..end+2).collect()).map_err(|_|error("Router SSE event is not UTF-8"))?;
                        for line in frame.lines().filter_map(|line|line.strip_prefix("data:")) {
                            let event:serde_json::Value=serde_json::from_str(line.trim()).map_err(|e|error(&e.to_string()))?;
                            if event.get("model").and_then(|v|v.as_str()).is_none() || event.get("event").and_then(|v|v.as_str()).is_none() {return Err(error("Malformed router SSE event"));}
                            changed=true;
                        }
                    }
                    if changed {self.sample().await?;lease=tokio::time::Instant::now()+IDLE_LEASE;}
                }
            }
        }
    }
    async fn sample(&mut self) -> Result<()> {
        // A concurrent serving receipt settlement invalidates the complete sample.
        // Re-read rather than replaying a late external observation over it.
        for _ in 0..8 {
            let receipt = self.current()?;
            let cursor = self.context.serving.status().await.snapshot.cursor;
            let response = self.read_models(false).await?;
            let owner = self.owner()?;
            let (preset, busy, uncertain) =
                owner.router_observation_preset(&self.spec.profile_id, &receipt)?;
            if uncertain {
                self.sync.catalog_state = RouterCatalogState::Uncertain;
                return Err(error("Router model state is uncertain; restart required"));
            }
            let rows = snapshot::decode(response, &preset, &self.spec, &receipt)?;
            if self.catalog_dirty && !busy && self.reconcile_catalog(&receipt).await? {
                continue;
            }
            self.sync.observation_state = RouterObservationState::Current;
            self.sync.catalog_state =
                if self.sync.pending_model_ids.is_empty() && !self.catalog_dirty {
                    RouterCatalogState::Current
                } else {
                    RouterCatalogState::Pending
                };
            self.sync.last_error = None;
            if self.context.serving.publish_router_observation(
                &self.spec.profile_id,
                &receipt,
                &owner,
                &cursor,
                rows,
                self.sync.clone(),
            )? {
                return Ok(());
            }
        }
        Err(error(
            "Serving operations changed during router observation",
        ))
    }
    async fn reconcile_catalog(
        &mut self,
        receipt: &OwnedRuntimeProfileObservation,
    ) -> Result<bool> {
        let desired = generate_llama_cpp_router_catalog(self.context.library.clone()).await?;
        let owner = self.owner()?;
        let (preset, busy, uncertain) =
            owner.router_observation_preset(&self.spec.profile_id, receipt)?;
        if busy {
            return Ok(false);
        }
        if uncertain {
            self.sync.catalog_state = RouterCatalogState::Uncertain;
            return Err(error("Router model state is uncertain"));
        }
        let (replacement, pending) =
            catalog::merge_additions(&preset, desired.preset_ini.as_bytes())?;
        self.sync.pending_model_ids = pending;
        if replacement == preset {
            self.catalog_dirty = false;
            return Ok(false);
        }
        let mut operation = match owner.begin_router_model_operation(&self.spec.profile_id, receipt)
        {
            Ok(operation) => operation,
            Err(err) => {
                let (_, busy, _) =
                    owner.router_observation_preset(&self.spec.profile_id, receipt)?;
                if busy {
                    return Ok(false);
                }
                return Err(err);
            }
        };
        // Recompute against the exact version protected by this operation.
        let preset = operation.preset()?;
        let (replacement, pending) =
            catalog::merge_additions(&preset, desired.preset_ini.as_bytes())?;
        self.sync.pending_model_ids = pending;
        if replacement == preset {
            operation.finish()?;
            self.catalog_dirty = false;
            return Ok(false);
        }
        let result: Result<()> = async {
            operation
                .replace_preset(preset, replacement.clone())
                .await?;
            operation.mark_mutating()?;
            let value = self.read_models(true).await?;
            snapshot::decode(value, &replacement, &self.spec, receipt)?;
            Ok(())
        }
        .await;
        if let Err(err) = result {
            self.sync.catalog_state = RouterCatalogState::Uncertain;
            return Err(err);
        }
        operation.finish()?;
        self.catalog_dirty = false;
        Ok(true)
    }
    async fn read_models(&self, reload: bool) -> Result<serde_json::Value> {
        let url = format!(
            "{}/v1/models{}",
            self.spec.endpoint_url.as_str().trim_end_matches('/'),
            if reload { "?reload=1" } else { "" }
        );
        tokio::time::timeout(HTTP_BUDGET, async {
            let response = self
                .client
                .get(url)
                .send()
                .await
                .map_err(http_error)?
                .error_for_status()
                .map_err(http_error)?;
            if response
                .content_length()
                .is_some_and(|n| n > 8 * 1024 * 1024)
            {
                return Err(error("Router snapshot too large"));
            }
            let mut stream = response.bytes_stream();
            let mut bytes = Vec::new();
            while let Some(chunk) = stream.next().await {
                let chunk = chunk.map_err(http_error)?;
                if bytes.len() + chunk.len() > 8 * 1024 * 1024 {
                    return Err(error("Router snapshot too large"));
                }
                bytes.extend_from_slice(&chunk);
            }
            serde_json::from_slice(&bytes).map_err(|e| error(&e.to_string()))
        })
        .await
        .map_err(|_| error("Router model read timed out"))?
    }
}
fn error(message: &str) -> PumasError {
    PumasError::Other(message.into())
}
fn http_error(error: reqwest::Error) -> PumasError {
    PumasError::Other(error.to_string())
}
