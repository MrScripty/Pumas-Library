//! Generic audio routing accepts only an opaque, already owned native slot.
//! Registration retains a weak reference; it cannot create a runtime, select
//! bytes from a request, or promote an installed manifest into qualification.

use super::audio_channel::ChannelError;
use super::audio_client::{FinishReason, OwnedAudioSlot};
use crate::model_library::ModelLibrary;
use crate::models::RuntimeProfileId;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, Weak};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OwnedAudioEndpointError {
    NotAdmitted,
    InvalidRequest,
    UnsupportedContract,
    ProviderFailure,
    TransportLost,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnedAudioEndpointResult {
    pub text: String,
    /// Only the native adapter's confirmed stop/length evidence reaches here.
    pub length_limited: bool,
}

struct Endpoint {
    library: Arc<ModelLibrary>,
    profile: RuntimeProfileId,
    slot: OwnedAudioSlot,
}

/// An in-memory reference to the original loaded slot and owning channel.
/// No public constructor, deserialization, caller path, or native ID lookup.
#[derive(Clone)]
pub struct OwnedAudioEndpoint(Arc<Endpoint>);

impl std::fmt::Debug for OwnedAudioEndpoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OwnedAudioEndpoint")
            .field("profile", &self.0.profile)
            .field("model", &self.0.slot.model_id())
            .finish_non_exhaustive()
    }
}

impl OwnedAudioEndpoint {
    pub fn available(&self) -> bool {
        self.0.slot.available()
    }

    /// This bounded closed request is revalidated by the owning native bridge.
    /// Caller loss drops its waiter; the original status observer keeps custody.
    pub async fn execute(
        &self,
        mut request: Value,
        admission: Arc<AtomicBool>,
    ) -> Result<OwnedAudioEndpointResult, OwnedAudioEndpointError> {
        if request.get("model").and_then(Value::as_str) != Some(self.0.slot.model_id())
            || request.get("profile").and_then(Value::as_str) != Some(self.0.profile.as_str())
            || !self.available()
        {
            return Err(OwnedAudioEndpointError::NotAdmitted);
        }
        // Profile authority is the retained slot. The private native bridge
        // has no profile selector; do not forward a new routing claim to it.
        request
            .as_object_mut()
            .ok_or(OwnedAudioEndpointError::NotAdmitted)?
            .remove("profile");
        let operation = self
            .0
            .slot
            .client()
            .start_with_admission(&self.0.slot, request, Some(admission))
            .await
            .map_err(|error| match error {
                ChannelError::InvalidRequest => OwnedAudioEndpointError::InvalidRequest,
                ChannelError::UnsupportedContract => OwnedAudioEndpointError::UnsupportedContract,
                ChannelError::NotAdmitted | ChannelError::NativeRejected | ChannelError::Limit => {
                    OwnedAudioEndpointError::NotAdmitted
                }
                _ => OwnedAudioEndpointError::TransportLost,
            })?;
        let result = operation.wait().await.map_err(|error| match error {
            ChannelError::NativeRejected => OwnedAudioEndpointError::ProviderFailure,
            _ => OwnedAudioEndpointError::TransportLost,
        })?;
        Ok(OwnedAudioEndpointResult {
            text: result.text,
            length_limited: result.finish_reason == FinishReason::Length,
        })
    }
}

#[derive(Default)]
pub(crate) struct AudioEndpoints {
    entries: Mutex<HashMap<(RuntimeProfileId, String), Weak<Endpoint>>>,
}
impl std::fmt::Debug for AudioEndpoints {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AudioEndpoints").finish_non_exhaustive()
    }
}
impl AudioEndpoints {
    // Only a qualified load's owning producer may call this. The shipping
    // runtime factory currently refuses that load; byte retention is not proof.
    #[allow(dead_code)]
    pub(crate) fn register(
        &self,
        library: Arc<ModelLibrary>,
        slot: OwnedAudioSlot,
    ) -> crate::Result<OwnedAudioEndpoint> {
        let profile = slot.profile_id().clone();
        let mut entries = self.entries.lock().map_err(|_| {
            crate::PumasError::Other("Owned audio registration is unavailable".into())
        })?;
        let key = (profile.clone(), slot.model_id().to_owned());
        entries.retain(|_, endpoint| endpoint.strong_count() != 0);
        if entries.get(&key).and_then(Weak::upgrade).is_some() || !slot.available() {
            return Err(crate::PumasError::Other(
                "Owned audio slot is unavailable".into(),
            ));
        }
        let endpoint = Arc::new(Endpoint {
            library,
            profile,
            slot,
        });
        entries.insert(key, Arc::downgrade(&endpoint));
        Ok(OwnedAudioEndpoint(endpoint))
    }

    pub(crate) fn selected(
        &self,
        library: &Arc<ModelLibrary>,
        profile: &RuntimeProfileId,
        model: &str,
    ) -> Option<OwnedAudioEndpoint> {
        let endpoint = self
            .entries
            .lock()
            .ok()?
            .get(&(profile.clone(), model.into()))?
            .upgrade()?;
        (Arc::ptr_eq(library, &endpoint.library) && endpoint.slot.available())
            .then_some(OwnedAudioEndpoint(endpoint))
    }
}

/// Controlled process evidence only; absent without explicit test-support.
/// The fixture reads synthetic selected bytes, never tensors or real ASR.
#[cfg(all(feature = "test-support", target_os = "linux"))]
pub struct ControlledAudioEndpointFixture {
    fixture: super::audio_client::fixture::Fixture,
    endpoint: OwnedAudioEndpoint,
}
#[cfg(all(feature = "test-support", target_os = "linux"))]
impl ControlledAudioEndpointFixture {
    pub async fn wait_use_started(&mut self) {
        use tokio::io::AsyncBufReadExt;
        let mut line = String::new();
        self.fixture.stderr.read_line(&mut line).await.unwrap();
        assert_eq!(line.trim(), "controlled use entered");
    }

    pub async fn wait_idle(&self) {
        self.endpoint.0.slot.wait_idle().await;
    }
    pub async fn launch(api: &crate::PumasApi, length: bool, hold_until_cancel: bool) -> Self {
        let flags = if hold_until_cancel {
            vec!["--hold-use-until-cancel"]
        } else {
            vec![]
        };
        let tokens = if length { vec![1; 512] } else { vec![0] };
        let mut fixture = super::audio_client::fixture::Fixture::launch_in_library(
            &tokens,
            &flags,
            Some(api.model_library().clone()),
        )
        .await;
        let slot = fixture.load().await;
        let endpoint = api
            .primary()
            .runtime_profile_service
            .audio_endpoints
            .register(api.model_library().clone(), slot)
            .unwrap();
        Self { fixture, endpoint }
    }

    pub async fn unload(&self) {
        self.endpoint
            .0
            .slot
            .client()
            .unload(&self.endpoint.0.slot)
            .await
            .unwrap();
    }

    pub async fn stop(&self) {
        self.fixture.stop().await;
    }
}
