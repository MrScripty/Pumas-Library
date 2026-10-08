//! Private exact-child framed transport. Public request futures never own pipe reads.
//!
//! One retained reader drains original replies; one writer serializes frames.
//! A blocking settlement exchange can coexist with finite cancellation exchanges.
//! Qualification is supplied by the owning runtime, never decoded from a reply.

#![allow(dead_code)] // Production runtime qualification remains unavailable.

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{value::RawValue, Value};
use std::any::Any;
use std::collections::HashMap;
use std::io;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::{mpsc, oneshot, Notify, OwnedSemaphorePermit, Semaphore};

pub(crate) const MAX_REQUEST: usize = 32 * 1024 * 1024;
pub(crate) const MAX_REPLY: usize = 64 * 1024;
const MAX_PENDING: usize = 64;
pub(crate) type Output = Box<dyn Any + Send>;
pub(crate) type Result<T> = std::result::Result<T, ChannelError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ChannelError {
    NotAdmitted,
    NativeRejected,
    InvalidRequest,
    UnsupportedContract,
    Unknown,
    Incoherent,
    Limit,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct NativeError {
    pub(crate) code: String,
    pub(crate) effect: NativeEffect,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum NativeEffect {
    NotAdmitted,
    Unknown,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    version: u32,
    exchange_id: u64,
    operation: String,
    #[serde(default)]
    result: Option<Box<RawValue>>,
    #[serde(default)]
    error: Option<NativeError>,
}

pub(crate) enum NativeReply {
    Result(Box<RawValue>),
    Error(NativeError),
}

impl NativeReply {
    pub(crate) fn decode<T: DeserializeOwned>(self) -> Result<T> {
        match self {
            Self::Result(value) => {
                serde_json::from_str(value.get()).map_err(|_| ChannelError::Incoherent)
            }
            Self::Error(error) => Err(if error.effect == NativeEffect::NotAdmitted {
                ChannelError::NativeRejected
            } else {
                ChannelError::Unknown
            }),
        }
    }
}

/// Concrete retained custody is captured before erasure. Admission happens only
/// after the final closed-waiter check; Drop after admission preserves uncertainty.
pub(crate) trait ExchangeCustody: Send {
    fn admit(&mut self) -> Result<()>;
    fn complete(self: Box<Self>, reply: NativeReply) -> Result<Output>;
}

type Prepare = Box<dyn FnOnce() -> Result<Box<dyn ExchangeCustody>> + Send>;

struct Job {
    operation: &'static str,
    payload: Value,
    prepare: Prepare,
    reply: oneshot::Sender<Result<Output>>,
    cancellation: Arc<ExchangeCancellation>,
}

struct Pending {
    operation: &'static str,
    custody: Box<dyn ExchangeCustody>,
    reply: oneshot::Sender<Result<Output>>,
    _permit: OwnedSemaphorePermit,
    write_finished: oneshot::Receiver<()>,
}

struct Shared {
    closed: AtomicBool,
    changed: Notify,
    pending: Mutex<HashMap<u64, Pending>>,
    permits: Arc<Semaphore>,
    next_id: AtomicU64,
    stop_exact_child: Arc<dyn Fn() + Send + Sync>,
}

impl Shared {
    fn quarantine(&self) {
        if self.closed.swap(true, Ordering::AcqRel) {
            return;
        }
        // These are all admitted original exchanges. Dropping their concrete
        // custody does not release unknown native effects or selected bytes.
        if let Ok(mut pending) = self.pending.lock() {
            for (_, item) in pending.drain() {
                let _ = item.reply.send(Err(ChannelError::Unknown));
            }
        }
        self.permits.close();
        self.changed.notify_waiters();
        (self.stop_exact_child)();
    }
}

struct WorkerExit(Arc<Shared>);
impl Drop for WorkerExit {
    fn drop(&mut self) {
        self.0.quarantine();
    }
}

#[derive(Default)]
struct ExchangeCancellation {
    wire_id: AtomicU64,
    requested: AtomicBool,
    admitted: AtomicBool,
    sent: AtomicBool,
}

pub(crate) struct PrivateAudioChannel {
    queue: mpsc::Sender<Job>,
    shared: Arc<Shared>,
}

impl PrivateAudioChannel {
    pub(crate) fn quarantine(&self) {
        self.shared.quarantine();
    }
    pub(crate) fn from_pipes(
        input: std::process::ChildStdin,
        output: std::process::ChildStdout,
        stop_exact_child: Arc<dyn Fn() + Send + Sync>,
    ) -> io::Result<Arc<Self>> {
        Ok(Self::from_streams(
            tokio::process::ChildStdin::from_std(input)?,
            tokio::process::ChildStdout::from_std(output)?,
            stop_exact_child,
        ))
    }

    fn from_streams<W, R>(
        writer: W,
        reader: R,
        stop_exact_child: Arc<dyn Fn() + Send + Sync>,
    ) -> Arc<Self>
    where
        W: AsyncWrite + Unpin + Send + 'static,
        R: AsyncRead + Unpin + Send + 'static,
    {
        let (queue, receiver) = mpsc::channel(1);
        let shared = Arc::new(Shared {
            closed: AtomicBool::new(false),
            changed: Notify::new(),
            pending: Mutex::new(HashMap::new()),
            permits: Arc::new(Semaphore::new(MAX_PENDING)),
            next_id: AtomicU64::new(1),
            stop_exact_child,
        });
        let channel = Arc::new(Self {
            queue,
            shared: shared.clone(),
        });
        tokio::spawn(read_replies(reader, shared.clone()));
        tokio::spawn(write_requests(
            writer,
            receiver,
            shared,
            Arc::downgrade(&channel),
        ));
        channel
    }

    pub(crate) async fn exchange<T: Send + 'static>(
        self: &Arc<Self>,
        operation: &'static str,
        payload: Value,
        prepare: Prepare,
    ) -> Result<T> {
        if self.shared.closed.load(Ordering::Acquire) {
            return Err(ChannelError::NotAdmitted);
        }
        let cancellation = Arc::new(ExchangeCancellation::default());
        let mut guard = CallerLoss {
            channel: self.clone(),
            operation,
            cancellation: cancellation.clone(),
            armed: true,
        };
        let (reply, result) = oneshot::channel();
        self.queue
            .send(Job {
                operation,
                payload,
                prepare,
                reply,
                cancellation,
            })
            .await
            .map_err(|_| ChannelError::NotAdmitted)?;
        let result = result.await.map_err(|_| ChannelError::Unknown)?;
        guard.armed = false;
        let result = result?;
        result
            .downcast::<T>()
            .map(|value| *value)
            .map_err(|_| ChannelError::Incoherent)
    }

    fn cancel_original(self: &Arc<Self>, cancellation: &ExchangeCancellation) {
        if !cancellation.admitted.load(Ordering::Acquire)
            || cancellation.sent.swap(true, Ordering::AcqRel)
            || self.shared.closed.load(Ordering::Acquire)
        {
            return;
        }
        let id = cancellation.wire_id.load(Ordering::Acquire);
        let queue = self.queue.clone();
        tokio::spawn(async move {
            // Cancellation controls have their own original IDs. Their finite
            // error cannot replay or retarget the original load admission.
            let (reply, result) = oneshot::channel();
            let _ = queue
                .send(Job {
                    operation: "cancel_exchange",
                    payload: serde_json::json!({"target_exchange_id":id}),
                    prepare: Box::new(move || Ok(Box::new(CancelExchangeCustody { target: id }))),
                    reply,
                    cancellation: Arc::new(ExchangeCancellation::default()),
                })
                .await;
            drop(queue);
            let _ = result.await;
        });
    }
}

struct CallerLoss {
    channel: Arc<PrivateAudioChannel>,
    operation: &'static str,
    cancellation: Arc<ExchangeCancellation>,
    armed: bool,
}
impl Drop for CallerLoss {
    fn drop(&mut self) {
        if self.armed && self.operation == "load" {
            self.cancellation.requested.store(true, Ordering::Release);
            self.channel.cancel_original(&self.cancellation);
        }
    }
}

pub(crate) struct ValueCustody;
impl ExchangeCustody for ValueCustody {
    fn admit(&mut self) -> Result<()> {
        Ok(())
    }
    fn complete(self: Box<Self>, reply: NativeReply) -> Result<Output> {
        Ok(Box::new(reply.decode::<Value>()?))
    }
}

struct CancelExchangeCustody {
    target: u64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CancelExchangeReply {
    target_exchange_id: u64,
    cancellation_requested: bool,
}
impl ExchangeCustody for CancelExchangeCustody {
    fn admit(&mut self) -> Result<()> {
        Ok(())
    }
    fn complete(self: Box<Self>, reply: NativeReply) -> Result<Output> {
        let reply: CancelExchangeReply = reply.decode()?;
        if reply.target_exchange_id != self.target || !reply.cancellation_requested {
            return Err(ChannelError::Incoherent);
        }
        Ok(Box::new(()))
    }
}

#[derive(Serialize)]
struct Request<'a> {
    version: u32,
    exchange_id: u64,
    operation: &'a str,
    payload: Value,
}

async fn write_requests<W: AsyncWrite + Unpin>(
    mut writer: W,
    mut queue: mpsc::Receiver<Job>,
    shared: Arc<Shared>,
    channel: std::sync::Weak<PrivateAudioChannel>,
) {
    let _exit = WorkerExit(shared.clone());
    loop {
        let changed = shared.changed.notified();
        tokio::pin!(changed);
        changed.as_mut().enable();
        if shared.closed.load(Ordering::Acquire) {
            return;
        }
        let job = tokio::select! { biased; _=&mut changed=>continue, job=queue.recv()=>match job {Some(job)=>job,None=>return} };
        if job.reply.is_closed() {
            continue;
        }
        let permit = match shared.permits.clone().acquire_owned().await {
            Ok(permit) => permit,
            Err(_) => return,
        };
        if job.reply.is_closed() || shared.closed.load(Ordering::Acquire) {
            continue;
        }
        let Ok(id) = shared
            .next_id
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |id| id.checked_add(1))
        else {
            return;
        };
        job.cancellation.wire_id.store(id, Ordering::Release);
        let bytes = match serde_json::to_vec(&Request {
            version: 1,
            exchange_id: id,
            operation: job.operation,
            payload: job.payload,
        }) {
            Ok(bytes) if !bytes.is_empty() && bytes.len() <= MAX_REQUEST => bytes,
            _ => {
                let _ = job.reply.send(Err(ChannelError::Limit));
                continue;
            }
        };
        let mut custody = match tokio::task::spawn_blocking(job.prepare).await {
            Ok(Ok(custody)) => custody,
            Ok(Err(error)) => {
                let _ = job.reply.send(Err(error));
                continue;
            }
            Err(_) => {
                let _ = job.reply.send(Err(ChannelError::Unknown));
                return;
            }
        };
        if job.reply.is_closed() || shared.closed.load(Ordering::Acquire) {
            continue;
        }
        let (written, write_finished) = oneshot::channel();
        {
            let Ok(mut pending) = shared.pending.lock() else {
                return;
            };
            if let Err(error) = custody.admit() {
                let _ = job.reply.send(Err(error));
                continue;
            }
            job.cancellation.admitted.store(true, Ordering::Release);
            pending.insert(
                id,
                Pending {
                    operation: job.operation,
                    custody,
                    reply: job.reply,
                    _permit: permit,
                    write_finished,
                },
            );
        }
        // No request cancellation can abandon a partial write or a reply read.
        if writer
            .write_all(&(bytes.len() as u32).to_be_bytes())
            .await
            .is_err()
            || writer.write_all(&bytes).await.is_err()
            || writer.flush().await.is_err()
        {
            return;
        }
        let _ = written.send(());
        if job.operation == "load" && job.cancellation.requested.load(Ordering::Acquire) {
            if let Some(channel) = channel.upgrade() {
                channel.cancel_original(&job.cancellation);
            }
        }
    }
}

async fn read_replies<R: AsyncRead + Unpin>(mut reader: R, shared: Arc<Shared>) {
    let _exit = WorkerExit(shared.clone());
    loop {
        let mut header = [0; 4];
        if reader.read_exact(&mut header).await.is_err() {
            return;
        }
        let size = u32::from_be_bytes(header) as usize;
        if size == 0 || size > MAX_REPLY {
            return;
        }
        let mut bytes = vec![0; size];
        if reader.read_exact(&mut bytes).await.is_err() {
            return;
        }
        let Ok(unique) = serde_json::from_slice::<UniqueValue>(&bytes) else {
            return;
        };
        let Some(keys) = unique.0.as_object() else {
            return;
        };
        if keys.contains_key("result") == keys.contains_key("error") {
            return;
        }
        let Ok(reply) = serde_json::from_slice::<Envelope>(&bytes) else {
            return;
        };
        if reply.version != 1 || reply.result.is_some() == reply.error.is_some() {
            return;
        }
        let pending = {
            let Ok(mut map) = shared.pending.lock() else {
                return;
            };
            map.remove(&reply.exchange_id)
        };
        let Some(pending) = pending else {
            return;
        };
        if pending.operation != reply.operation {
            let _ = pending.reply.send(Err(ChannelError::Incoherent));
            return;
        }
        let reply = match (reply.result, reply.error) {
            (Some(value), None) => NativeReply::Result(value),
            (None, Some(error))
                if !error.code.is_empty()
                    && error.code.len() <= 128
                    && error
                        .code
                        .bytes()
                        .all(|b| b.is_ascii_lowercase() || b == b'_') =>
            {
                NativeReply::Error(error)
            }
            _ => {
                let _ = pending.reply.send(Err(ChannelError::Incoherent));
                return;
            }
        };
        if pending.write_finished.await.is_err() {
            let _ = pending.reply.send(Err(ChannelError::Unknown));
            return;
        }
        let result = pending.custody.complete(reply);
        let incoherent = matches!(
            result,
            Err(ChannelError::Incoherent | ChannelError::Unknown)
        );
        let _ = pending.reply.send(result);
        if incoherent {
            return;
        }
    }
}

// Reject duplicate keys at every depth, including generic control results. The
// bounded reply is then decoded again into its original closed typed receipt.
struct UniqueValue(Value);
impl<'de> Deserialize<'de> for UniqueValue {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = UniqueValue;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("JSON without duplicate keys")
            }
            fn visit_bool<E: serde::de::Error>(
                self,
                value: bool,
            ) -> std::result::Result<Self::Value, E> {
                Ok(UniqueValue(Value::Bool(value)))
            }
            fn visit_i64<E: serde::de::Error>(
                self,
                value: i64,
            ) -> std::result::Result<Self::Value, E> {
                Ok(UniqueValue(Value::Number(value.into())))
            }
            fn visit_u64<E: serde::de::Error>(
                self,
                value: u64,
            ) -> std::result::Result<Self::Value, E> {
                Ok(UniqueValue(Value::Number(value.into())))
            }
            fn visit_f64<E: serde::de::Error>(
                self,
                value: f64,
            ) -> std::result::Result<Self::Value, E> {
                serde_json::Number::from_f64(value)
                    .map(|value| UniqueValue(Value::Number(value)))
                    .ok_or_else(|| E::custom("invalid JSON number"))
            }
            fn visit_str<E: serde::de::Error>(
                self,
                value: &str,
            ) -> std::result::Result<Self::Value, E> {
                Ok(UniqueValue(Value::String(value.into())))
            }
            fn visit_string<E: serde::de::Error>(
                self,
                value: String,
            ) -> std::result::Result<Self::Value, E> {
                Ok(UniqueValue(Value::String(value)))
            }
            fn visit_unit<E: serde::de::Error>(self) -> std::result::Result<Self::Value, E> {
                Ok(UniqueValue(Value::Null))
            }
            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut sequence: A,
            ) -> std::result::Result<Self::Value, A::Error> {
                let mut values = Vec::new();
                while let Some(value) = sequence.next_element::<UniqueValue>()? {
                    values.push(value.0);
                }
                Ok(UniqueValue(Value::Array(values)))
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut map: A,
            ) -> std::result::Result<Self::Value, A::Error> {
                let mut values = serde_json::Map::new();
                while let Some(key) = map.next_key::<String>()? {
                    if values.contains_key(&key) {
                        return Err(serde::de::Error::custom("duplicate JSON key"));
                    }
                    values.insert(key, map.next_value::<UniqueValue>()?.0);
                }
                Ok(UniqueValue(Value::Object(values)))
            }
        }
        deserializer.deserialize_any(Visitor)
    }
}

#[cfg(test)]
#[path = "audio_channel/tests.rs"]
mod tests;
