//! Owning private native load/use/unload, under concrete selected-byte custody.
#![allow(dead_code)] // Installed runtime/read-set qualification is still closed.

use super::audio_channel::{
    ChannelError, ExchangeCustody, NativeEffect, NativeReply, Output, PrivateAudioChannel, Result,
};
use super::audio_custody::{
    AudioCustodyRegistry, AudioLoadAdmission, AudioLoadedSlot, AudioOperationBorrow,
    AudioOperationIdentity, AudioSlotIdentity, AudioUnloadAdmission,
};
use crate::model_library::artifact_use::PreparedArtifactUse;
use crate::models::RuntimeProfileId;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Weak};
use tokio::sync::Notify;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Slot {
    slot_id: String,
    load_generation: String,
}
impl From<&AudioSlotIdentity> for Slot {
    fn from(identity: &AudioSlotIdentity) -> Self {
        Self {
            slot_id: identity.slot_id.clone(),
            load_generation: identity.load_generation.clone(),
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Hello {
    runtime_instance_id: String,
    production_available: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SlotReply {
    runtime_instance_id: String,
    slot: Slot,
    state: String,
    cleanup: String,
    error_code: Option<String>,
}
impl SlotReply {
    fn validate(&self) -> Result<()> {
        if !super::audio_custody::valid_slot(&self.identity())
            || self.error_code.as_ref().is_some_and(|code| {
                code.is_empty()
                    || code.len() > 128
                    || !code
                        .bytes()
                        .all(|byte| byte.is_ascii_lowercase() || byte == b'_')
            })
        {
            return Err(ChannelError::Incoherent);
        }
        Ok(())
    }
    fn identity(&self) -> AudioSlotIdentity {
        AudioSlotIdentity {
            runtime_instance: self.runtime_instance_id.clone(),
            slot_id: self.slot.slot_id.clone(),
            load_generation: self.slot.load_generation.clone(),
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UseReply {
    runtime_instance_id: String,
    slot: Slot,
    operation_id: String,
    state: String,
    cleanup: String,
    text: Option<String>,
    finish_reason: Option<FinishReason>,
    error_code: Option<String>,
}
impl UseReply {
    fn identity(&self) -> AudioOperationIdentity {
        AudioOperationIdentity {
            slot: AudioSlotIdentity {
                runtime_instance: self.runtime_instance_id.clone(),
                slot_id: self.slot.slot_id.clone(),
                load_generation: self.slot.load_generation.clone(),
            },
            operation_id: self.operation_id.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum FinishReason {
    Stop,
    Length,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct NativeAudioResult {
    pub(crate) text: String,
    pub(crate) finish_reason: FinishReason,
}

pub(crate) struct OwnedAudioClient {
    channel: Arc<PrivateAudioChannel>,
    registry: Arc<AudioCustodyRegistry>,
    runtime: String,
}
impl OwnedAudioClient {
    pub(crate) async fn bind(
        channel: Arc<PrivateAudioChannel>,
        registry: Arc<AudioCustodyRegistry>,
        profile: RuntimeProfileId,
        generation: u64,
        pid: u32,
    ) -> Result<Arc<Self>> {
        let retained = registry.clone();
        let runtime = channel
            .exchange::<String>(
                "hello",
                json!({}),
                Box::new(move || {
                    Ok(Box::new(Custody::Hello {
                        registry: retained,
                        profile,
                        generation,
                        pid,
                    }))
                }),
            )
            .await?;
        Ok(Arc::new(Self {
            channel,
            registry,
            runtime,
        }))
    }

    pub(crate) async fn load(
        self: &Arc<Self>,
        prepared: Arc<PreparedArtifactUse>,
        source_id: &str,
    ) -> Result<OwnedAudioSlot> {
        let registry = self.registry.clone();
        let runtime = self.runtime.clone();
        let client = Arc::downgrade(self);
        let model = prepared.model_id().to_owned();
        self.channel
            .exchange(
                "load",
                json!({"runtime_instance_id":self.runtime,"model_id":model,"source_id":source_id}),
                Box::new(move || {
                    let admission = registry
                        .reserve_prepared(prepared)
                        .map_err(|_| ChannelError::NotAdmitted)?;
                    Ok(Box::new(Custody::Load {
                        admission,
                        runtime,
                        client,
                        model,
                    }))
                }),
            )
            .await
    }

    pub(crate) async fn start(
        self: &Arc<Self>,
        slot: &OwnedAudioSlot,
        request: Value,
    ) -> Result<OwnedAudioOperation> {
        self.start_with_admission(slot, request, None).await
    }

    pub(crate) async fn start_with_admission(
        self: &Arc<Self>,
        slot: &OwnedAudioSlot,
        request: Value,
        marker: Option<Arc<AtomicBool>>,
    ) -> Result<OwnedAudioOperation> {
        if !Arc::ptr_eq(self, &slot.owner.client) {
            return Err(ChannelError::NotAdmitted);
        }
        let owner = slot.owner.clone();
        let client = Arc::downgrade(self);
        let identity = owner.slot.identity();
        self.channel.exchange("use",json!({"runtime_instance_id":self.runtime,"slot":Slot::from(identity),"request":request}),Box::new(move||Ok(Box::new(Custody::Use {owner,borrow:None,client,marker})))).await
    }

    pub(crate) async fn execute(
        self: &Arc<Self>,
        slot: &OwnedAudioSlot,
        request: Value,
    ) -> Result<NativeAudioResult> {
        self.start(slot, request).await?.wait().await
    }

    pub(crate) async fn unload(self: &Arc<Self>, slot: &OwnedAudioSlot) -> Result<()> {
        if !Arc::ptr_eq(self, &slot.owner.client) {
            return Err(ChannelError::NotAdmitted);
        }
        self.unload_owner(slot.owner.slot.clone(), slot.owner.retired.clone())
            .await
    }

    async fn unload_owner(
        self: &Arc<Self>,
        slot: Arc<AudioLoadedSlot>,
        retired: Arc<AtomicBool>,
    ) -> Result<()> {
        if retired.load(Ordering::Acquire) {
            return Ok(());
        }
        let identity = slot.identity().clone();
        self.channel
            .exchange(
                "unload",
                json!({"runtime_instance_id":self.runtime,"slot":Slot::from(&identity)}),
                Box::new(move || {
                    let admission = slot.begin_unload().map_err(|_| ChannelError::NotAdmitted)?;
                    Ok(Box::new(Custody::Unload {
                        admission,
                        identity,
                        retired,
                    }))
                }),
            )
            .await
    }

    fn observe(self: &Arc<Self>, operation: Arc<Operation>) {
        let client = self.clone();
        tokio::spawn(async move {
            let retained = operation.clone();
            let result = client
                .channel
                .exchange::<()>(
                    "status",
                    operation.payload(true),
                    Box::new(move || {
                        Ok(Box::new(Custody::Observe {
                            operation: retained,
                        }))
                    }),
                )
                .await;
            if let Err(error) = result {
                operation.publish(Err(error));
                client.channel.quarantine();
            }
        });
    }

    fn request_cancel(self: &Arc<Self>, operation: Arc<Operation>) {
        if operation.is_settled() || operation.cancel_requested.swap(true, Ordering::AcqRel) {
            return;
        }
        let client = self.clone();
        tokio::spawn(async move {
            if operation.is_settled() {
                return;
            }
            let retained = operation.clone();
            let result = client
                .channel
                .exchange::<()>(
                    "cancel",
                    operation.payload(false),
                    Box::new(move || {
                        Ok(Box::new(Custody::Cancel {
                            operation: retained,
                        }))
                    }),
                )
                .await;
            if let Err(error) = result {
                if error == ChannelError::NotAdmitted && operation.is_settled() {
                    return;
                }
                operation.publish(Err(error));
                client.channel.quarantine();
            }
        });
    }
}

#[derive(Clone)]
pub(crate) struct OwnedAudioSlot {
    owner: Arc<SlotOwner>,
}
impl std::fmt::Debug for OwnedAudioSlot {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_tuple("OwnedAudioSlot")
            .field(self.identity())
            .finish()
    }
}
impl OwnedAudioSlot {
    #[cfg(any(test, feature = "test-support"))]
    pub(crate) async fn wait_idle(&self) {
        self.owner.slot.wait_idle().await.unwrap();
    }
    pub(crate) fn profile_id(&self) -> &RuntimeProfileId {
        self.owner.slot.profile_id()
    }
    pub(crate) fn model_id(&self) -> &str {
        &self.owner.model
    }

    pub(crate) fn client(&self) -> &Arc<OwnedAudioClient> {
        &self.owner.client
    }
    pub(crate) fn available(&self) -> bool {
        !self.owner.retired.load(Ordering::Acquire) && self.owner.slot.available()
    }
    pub(crate) fn identity(&self) -> &AudioSlotIdentity {
        self.owner.slot.identity()
    }
}
struct SlotOwner {
    slot: Arc<AudioLoadedSlot>,
    client: Arc<OwnedAudioClient>,
    retired: Arc<AtomicBool>,
    model: String,
}
impl Drop for SlotOwner {
    fn drop(&mut self) {
        if self.retired.load(Ordering::Acquire) {
            return;
        }
        let client = self.client.clone();
        let slot = self.slot.clone();
        let retired = self.retired.clone();
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            runtime.spawn(async move {
                if client.unload_owner(slot, retired).await.is_err() {
                    client.channel.quarantine();
                }
            });
        } else {
            client.channel.quarantine();
        }
    }
}

struct Operation {
    identity: AudioOperationIdentity,
    borrow: Mutex<Option<AudioOperationBorrow>>,
    result: Mutex<Option<Result<NativeAudioResult>>>,
    changed: Notify,
    cancel_requested: AtomicBool,
    _slot_owner: Arc<SlotOwner>,
}
impl Operation {
    fn payload(&self, wait: bool) -> Value {
        let mut result = json!({"runtime_instance_id":self.identity.slot.runtime_instance,"slot":Slot::from(&self.identity.slot),"operation_id":self.identity.operation_id});
        if wait {
            result["wait_for_settlement"] = json!(true);
        }
        result
    }
    fn publish(&self, result: Result<NativeAudioResult>) {
        if let Ok(mut state) = self.result.lock() {
            if state.is_none() {
                *state = Some(result);
                self.changed.notify_waiters();
            }
        }
    }
    fn is_settled(&self) -> bool {
        self.result.lock().is_ok_and(|state| state.is_some())
    }
}

#[derive(Clone)]
pub(crate) struct OwnedAudioOperation {
    operation: Arc<Operation>,
    _caller: Arc<OperationCaller>,
}
struct OperationCaller {
    operation: Arc<Operation>,
    client: Arc<OwnedAudioClient>,
}
impl Drop for OperationCaller {
    fn drop(&mut self) {
        self.client.request_cancel(self.operation.clone());
    }
}
impl OwnedAudioOperation {
    pub(crate) fn cancel(&self) {
        self._caller.client.request_cancel(self.operation.clone());
    }
    pub(crate) async fn wait(&self) -> Result<NativeAudioResult> {
        let mut guard = WaitLoss {
            caller: self._caller.clone(),
            armed: true,
        };
        loop {
            let notified = self.operation.changed.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            if let Some(result) = self
                .operation
                .result
                .lock()
                .map_err(|_| ChannelError::Unknown)?
                .clone()
            {
                guard.armed = false;
                return result;
            }
            notified.await;
        }
    }
}

struct WaitLoss {
    caller: Arc<OperationCaller>,
    armed: bool,
}
impl Drop for WaitLoss {
    fn drop(&mut self) {
        if self.armed {
            self.caller
                .client
                .request_cancel(self.caller.operation.clone());
        }
    }
}

fn validate_operation_reply(reply: &UseReply) -> Result<()> {
    if let Some(code) = &reply.error_code {
        if code.is_empty()
            || code.len() > 128
            || !code
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte == b'_')
        {
            return Err(ChannelError::Incoherent);
        }
    }
    match reply.state.as_str() {
        "running" | "cancellation_requested"
            if reply.cleanup == "pending"
                && reply.text.is_none()
                && reply.finish_reason.is_none()
                && reply.error_code.is_none() =>
        {
            Ok(())
        }
        "completed"
            if reply.cleanup == "confirmed"
                && reply.text.as_ref().is_some_and(|text| text.len() <= 16_000)
                && reply.finish_reason.is_some()
                && reply.error_code.is_none() =>
        {
            Ok(())
        }
        "failed"
            if reply.cleanup == "confirmed"
                && reply.text.is_none()
                && reply.finish_reason.is_none()
                && reply.error_code.is_some() =>
        {
            Ok(())
        }
        "cancelled"
            if reply.cleanup == "confirmed"
                && reply.text.is_none()
                && reply.finish_reason.is_none() =>
        {
            Ok(())
        }
        "cleanup_unconfirmed" if reply.cleanup == "unconfirmed" => Err(ChannelError::Unknown),
        _ => Err(ChannelError::Incoherent),
    }
}

enum Custody {
    Hello {
        registry: Arc<AudioCustodyRegistry>,
        profile: RuntimeProfileId,
        generation: u64,
        pid: u32,
    },
    Load {
        admission: AudioLoadAdmission,
        runtime: String,
        client: Weak<OwnedAudioClient>,
        model: String,
    },
    Use {
        owner: Arc<SlotOwner>,
        borrow: Option<AudioOperationBorrow>,
        client: Weak<OwnedAudioClient>,
        marker: Option<Arc<AtomicBool>>,
    },
    Observe {
        operation: Arc<Operation>,
    },
    Cancel {
        operation: Arc<Operation>,
    },
    Unload {
        admission: AudioUnloadAdmission,
        identity: AudioSlotIdentity,
        retired: Arc<AtomicBool>,
    },
}
impl ExchangeCustody for Custody {
    fn admit(&mut self) -> Result<()> {
        match self {
            Self::Load { admission, .. } => admission
                .mark_wire_admitted()
                .map_err(|_| ChannelError::NotAdmitted),
            Self::Use {
                owner,
                borrow,
                marker,
                ..
            } => {
                *borrow = Some(
                    owner
                        .slot
                        .borrow_operation()
                        .map_err(|_| ChannelError::NotAdmitted)?,
                );
                if let Some(marker) = marker {
                    marker.store(true, Ordering::Release);
                }
                Ok(())
            }
            Self::Unload { admission, .. } => admission
                .mark_wire_admitted()
                .map_err(|_| ChannelError::NotAdmitted),
            Self::Cancel { operation } if operation.is_settled() => Err(ChannelError::NotAdmitted),
            _ => Ok(()),
        }
    }
    fn complete(self: Box<Self>, reply: NativeReply) -> Result<Output> {
        match *self {
            Self::Hello {
                registry,
                profile,
                generation,
                pid,
            } => {
                let hello: Hello = reply.decode()?;
                if hello.production_available {
                    return Err(ChannelError::Incoherent);
                }
                registry
                    .bind_native_instance(&profile, generation, pid, &hello.runtime_instance_id)
                    .map_err(|_| ChannelError::Incoherent)?;
                Ok(Box::new(hello.runtime_instance_id))
            }
            Self::Load {
                admission,
                runtime,
                client,
                model,
            } => {
                let reply = match reply {
                    NativeReply::Error(error) if error.effect == NativeEffect::NotAdmitted => {
                        admission
                            .finish_native_cleaned(&runtime)
                            .map_err(|_| ChannelError::Incoherent)?;
                        return Err(ChannelError::NativeRejected);
                    }
                    reply => reply.decode::<SlotReply>()?,
                };
                if reply.runtime_instance_id != runtime {
                    return Err(ChannelError::Incoherent);
                }
                reply.validate()?;
                if reply.state == "failed"
                    && reply.cleanup == "confirmed"
                    && reply.error_code.is_some()
                {
                    admission
                        .finish_native_cleaned(&runtime)
                        .map_err(|_| ChannelError::Incoherent)?;
                    return Err(ChannelError::NativeRejected);
                }
                if reply.state == "cleanup_unconfirmed" && reply.cleanup == "unconfirmed" {
                    return Err(ChannelError::Unknown);
                }
                if reply.state != "ready"
                    || reply.cleanup != "retained"
                    || reply.error_code.is_some()
                {
                    return Err(ChannelError::Incoherent);
                }
                let slot = Arc::new(
                    admission
                        .ready(reply.identity())
                        .map_err(|_| ChannelError::Incoherent)?,
                );
                let client = client.upgrade().ok_or(ChannelError::Unknown)?;
                Ok(Box::new(OwnedAudioSlot {
                    owner: Arc::new(SlotOwner {
                        slot,
                        client,
                        retired: Arc::new(AtomicBool::new(false)),
                        model,
                    }),
                }))
            }
            Self::Use {
                owner,
                borrow,
                client,
                marker: _,
            } => {
                let mut borrow = borrow.ok_or(ChannelError::Incoherent)?;
                let reply = match reply {
                    NativeReply::Error(error) if error.effect == NativeEffect::NotAdmitted => {
                        borrow
                            .finish_native_not_started(owner.slot.identity())
                            .map_err(|_| ChannelError::Incoherent)?;
                        return Err(match error.code.as_str() {
                            "invalid_request" => ChannelError::InvalidRequest,
                            "unsupported_contract" => ChannelError::UnsupportedContract,
                            _ => ChannelError::NativeRejected,
                        });
                    }
                    reply => reply.decode::<UseReply>()?,
                };
                let identity = reply.identity();
                if identity.slot != *owner.slot.identity() {
                    return Err(ChannelError::Incoherent);
                }
                validate_operation_reply(&reply)?;
                borrow
                    .bind_native_operation(&identity)
                    .map_err(|_| ChannelError::Incoherent)?;
                let operation = Arc::new(Operation {
                    identity,
                    borrow: Mutex::new(Some(borrow)),
                    result: Mutex::new(None),
                    changed: Notify::new(),
                    cancel_requested: AtomicBool::new(false),
                    _slot_owner: owner,
                });
                let client = client.upgrade().ok_or(ChannelError::Unknown)?;
                client.observe(operation.clone());
                let caller = Arc::new(OperationCaller {
                    operation: operation.clone(),
                    client,
                });
                Ok(Box::new(OwnedAudioOperation {
                    operation,
                    _caller: caller,
                }))
            }
            Self::Observe { operation } => {
                let reply: UseReply = reply.decode()?;
                if reply.identity() != operation.identity {
                    return Err(ChannelError::Incoherent);
                }
                validate_operation_reply(&reply)?;
                if reply.state == "cleanup_unconfirmed" && reply.cleanup == "unconfirmed" {
                    return Err(ChannelError::Unknown);
                }
                if reply.cleanup != "confirmed"
                    || !matches!(reply.state.as_str(), "completed" | "failed" | "cancelled")
                {
                    return Err(ChannelError::Incoherent);
                }
                let result = if reply.state == "completed" {
                    let text = reply.text.ok_or(ChannelError::Incoherent)?;
                    if text.len() > 16_000 || reply.error_code.is_some() {
                        return Err(ChannelError::Incoherent);
                    }
                    Ok(NativeAudioResult {
                        text,
                        finish_reason: reply.finish_reason.ok_or(ChannelError::Incoherent)?,
                    })
                } else {
                    if reply.text.is_some()
                        || reply.finish_reason.is_some()
                        || (reply.state == "failed" && reply.error_code.is_none())
                    {
                        return Err(ChannelError::Incoherent);
                    }
                    Err(ChannelError::NativeRejected)
                };
                let borrow = operation
                    .borrow
                    .lock()
                    .map_err(|_| ChannelError::Unknown)?
                    .take()
                    .ok_or(ChannelError::Incoherent)?;
                borrow
                    .finish_native_settled(&operation.identity)
                    .map_err(|_| ChannelError::Incoherent)?;
                operation.publish(result);
                Ok(Box::new(()))
            }
            Self::Cancel { operation } => {
                let reply: UseReply = reply.decode()?;
                if reply.identity() != operation.identity {
                    return Err(ChannelError::Incoherent);
                }
                validate_operation_reply(&reply)?;
                // An acknowledgement is not a settlement or release receipt.
                Ok(Box::new(()))
            }
            Self::Unload {
                admission,
                identity,
                retired,
            } => {
                let reply: SlotReply = match reply {
                    NativeReply::Error(error) if error.effect == NativeEffect::NotAdmitted => {
                        admission
                            .finish_native_not_started(&identity)
                            .map_err(|_| ChannelError::Incoherent)?;
                        return Err(ChannelError::NativeRejected);
                    }
                    reply => reply.decode()?,
                };
                if reply.identity() != identity {
                    return Err(ChannelError::Incoherent);
                }
                reply.validate()?;
                if reply.state == "cleanup_unconfirmed" && reply.cleanup == "unconfirmed" {
                    return Err(ChannelError::Unknown);
                }
                if reply.state != "retired"
                    || reply.cleanup != "confirmed"
                    || reply.error_code.is_some()
                {
                    return Err(ChannelError::Incoherent);
                }
                admission
                    .finish_native_unloaded(&identity)
                    .map_err(|_| ChannelError::Incoherent)?;
                retired.store(true, Ordering::Release);
                Ok(Box::new(()))
            }
        }
    }
}

#[cfg(all(any(test, feature = "test-support"), target_os = "linux"))]
#[path = "audio_client/fixture.rs"]
pub(crate) mod fixture;
#[cfg(all(test, target_os = "linux"))]
#[path = "audio_client/tests.rs"]
mod tests;
