use super::*;
use std::pin::Pin;
use std::sync::atomic::AtomicUsize;
use std::task::{Context, Poll};
use tokio::io::DuplexStream;

#[derive(Default)]
struct Effects {
    prepared: AtomicUsize,
    admitted: AtomicUsize,
    settled: AtomicUsize,
    uncertain: AtomicUsize,
    unadmitted: AtomicUsize,
    changed: Notify,
}

struct Custody {
    effects: Arc<Effects>,
    admitted: bool,
    settled: bool,
}

impl ExchangeCustody for Custody {
    fn admit(&mut self) -> Result<()> {
        self.admitted = true;
        self.effects.admitted.fetch_add(1, Ordering::AcqRel);
        Ok(())
    }

    fn complete(mut self: Box<Self>, reply: NativeReply) -> Result<Output> {
        let value = reply.decode::<Value>();
        if value.is_ok() || matches!(value, Err(ChannelError::NativeRejected)) {
            self.settled = true;
            self.effects.settled.fetch_add(1, Ordering::AcqRel);
        }
        value.map(|value| Box::new(value) as Output)
    }
}

impl Drop for Custody {
    fn drop(&mut self) {
        if !self.admitted {
            self.effects.unadmitted.fetch_add(1, Ordering::AcqRel);
        } else if !self.settled {
            self.effects.uncertain.fetch_add(1, Ordering::AcqRel);
        }
        self.effects.changed.notify_one();
    }
}

fn prepare(effects: &Arc<Effects>) -> Prepare {
    let effects = effects.clone();
    Box::new(move || {
        effects.prepared.fetch_add(1, Ordering::AcqRel);
        Ok(Box::new(Custody {
            effects,
            admitted: false,
            settled: false,
        }))
    })
}

fn gated_prepare(
    effects: &Arc<Effects>,
) -> (Prepare, oneshot::Receiver<()>, std::sync::mpsc::Sender<()>) {
    let effects = effects.clone();
    let (entered, entered_wait) = oneshot::channel();
    let (release, released) = std::sync::mpsc::channel();
    let prepare: Prepare = Box::new(move || {
        effects.prepared.fetch_add(1, Ordering::AcqRel);
        let _ = entered.send(());
        released.recv().map_err(|_| ChannelError::NotAdmitted)?;
        Ok(Box::new(Custody {
            effects,
            admitted: false,
            settled: false,
        }))
    });
    (prepare, entered_wait, release)
}

#[derive(Default)]
struct Stops {
    count: AtomicUsize,
    changed: Notify,
}

impl Stops {
    fn callback(self: &Arc<Self>) -> Arc<dyn Fn() + Send + Sync> {
        let stops = self.clone();
        Arc::new(move || {
            stops.count.fetch_add(1, Ordering::AcqRel);
            stops.changed.notify_one();
        })
    }

    async fn wait(&self) {
        while self.count.load(Ordering::Acquire) == 0 {
            self.changed.notified().await;
        }
    }
}

fn channel() -> (Arc<PrivateAudioChannel>, DuplexStream, Arc<Stops>) {
    let (client, peer) = tokio::io::duplex(128 * 1024);
    let (reader, writer) = tokio::io::split(client);
    let stops = Arc::new(Stops::default());
    (
        PrivateAudioChannel::from_streams(writer, reader, stops.callback()),
        peer,
        stops,
    )
}

fn call(
    channel: &Arc<PrivateAudioChannel>,
    operation: &'static str,
    preparation: Prepare,
) -> tokio::task::JoinHandle<Result<Value>> {
    let channel = channel.clone();
    tokio::spawn(async move {
        channel
            .exchange(operation, serde_json::json!({}), preparation)
            .await
    })
}

async fn read_request(peer: &mut DuplexStream) -> Value {
    let mut header = [0; 4];
    peer.read_exact(&mut header).await.unwrap();
    let length = u32::from_be_bytes(header) as usize;
    assert!((1..=MAX_REQUEST).contains(&length));
    let mut bytes = vec![0; length];
    peer.read_exact(&mut bytes).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

async fn send_bytes(peer: &mut DuplexStream, bytes: &[u8]) {
    peer.write_all(&(bytes.len() as u32).to_be_bytes())
        .await
        .unwrap();
    peer.write_all(bytes).await.unwrap();
    peer.flush().await.unwrap();
}

async fn reply(peer: &mut DuplexStream, original: &Value, result: Value) {
    send_bytes(
        peer,
        &serde_json::to_vec(&serde_json::json!({
            "version": 1,
            "exchange_id": original["exchange_id"],
            "operation": original["operation"],
            "result": result,
        }))
        .unwrap(),
    )
    .await;
}

async fn next_clean(channel: &Arc<PrivateAudioChannel>, peer: &mut DuplexStream) {
    let effects = Arc::new(Effects::default());
    let next = call(channel, "status", prepare(&effects));
    let frame = read_request(peer).await;
    assert_eq!(frame["operation"], "status");
    reply(peer, &frame, serde_json::json!({"clean": true})).await;
    assert_eq!(
        next.await.unwrap().unwrap(),
        serde_json::json!({"clean": true})
    );
    assert_eq!(effects.admitted.load(Ordering::Acquire), 1);
    assert_eq!(effects.settled.load(Ordering::Acquire), 1);
    assert_eq!(effects.uncertain.load(Ordering::Acquire), 0);
}

#[tokio::test]
async fn cancelled_queued_job_has_no_preparation_admission_or_wire_effect() {
    let (channel, mut peer, stops) = channel();
    let first_effects = Arc::new(Effects::default());
    let (preparation, entered, release) = gated_prepare(&first_effects);
    let first = call(&channel, "status", preparation);
    entered.await.unwrap();

    let cancelled_effects = Arc::new(Effects::default());
    let (reply_sender, result) = oneshot::channel();
    channel
        .queue
        .send(Job {
            operation: "load",
            payload: serde_json::json!({"cancelled": true}),
            prepare: prepare(&cancelled_effects),
            reply: reply_sender,
            cancellation: Arc::new(ExchangeCancellation::default()),
        })
        .await
        .unwrap();
    drop(result);
    release.send(()).unwrap();

    let frame = read_request(&mut peer).await;
    assert_eq!(frame["operation"], "status");
    reply(&mut peer, &frame, serde_json::json!({"first": true})).await;
    first.await.unwrap().unwrap();
    next_clean(&channel, &mut peer).await;
    assert_eq!(cancelled_effects.prepared.load(Ordering::Acquire), 0);
    assert_eq!(cancelled_effects.admitted.load(Ordering::Acquire), 0);
    assert_eq!(stops.count.load(Ordering::Acquire), 0);
}

#[tokio::test]
async fn caller_loss_during_preparation_is_rechecked_before_admission() {
    let (channel, mut peer, stops) = channel();
    let effects = Arc::new(Effects::default());
    let (preparation, entered, release) = gated_prepare(&effects);
    let cancelled = call(&channel, "load", preparation);
    entered.await.unwrap();
    cancelled.abort();
    assert!(cancelled.await.unwrap_err().is_cancelled());
    release.send(()).unwrap();
    next_clean(&channel, &mut peer).await;
    assert_eq!(effects.prepared.load(Ordering::Acquire), 1);
    assert_eq!(effects.admitted.load(Ordering::Acquire), 0);
    assert_eq!(effects.unadmitted.load(Ordering::Acquire), 1);
    assert_eq!(stops.count.load(Ordering::Acquire), 0);
}

#[tokio::test]
async fn admitted_caller_loss_retains_original_reply_and_allows_clean_next_exchange() {
    let (channel, mut peer, stops) = channel();
    let effects = Arc::new(Effects::default());
    let original = call(&channel, "status", prepare(&effects));
    let frame = read_request(&mut peer).await;
    original.abort();
    assert!(original.await.unwrap_err().is_cancelled());
    assert_eq!(effects.admitted.load(Ordering::Acquire), 1);
    assert_eq!(effects.settled.load(Ordering::Acquire), 0);
    assert_eq!(effects.uncertain.load(Ordering::Acquire), 0);
    reply(&mut peer, &frame, serde_json::json!({"original": true})).await;
    next_clean(&channel, &mut peer).await;
    assert_eq!(effects.settled.load(Ordering::Acquire), 1);
    assert_eq!(effects.uncertain.load(Ordering::Acquire), 0);
    assert_eq!(stops.count.load(Ordering::Acquire), 0);
}

#[tokio::test]
async fn lost_load_caller_sends_exact_original_cancel_and_drains_out_of_order_replies() {
    let (channel, mut peer, stops) = channel();
    let effects = Arc::new(Effects::default());
    let original = call(&channel, "load", prepare(&effects));
    let frame = read_request(&mut peer).await;
    original.abort();
    assert!(original.await.unwrap_err().is_cancelled());
    let cancellation = read_request(&mut peer).await;
    assert_eq!(cancellation["operation"], "cancel_exchange");
    assert_eq!(
        cancellation["payload"]["target_exchange_id"],
        frame["exchange_id"]
    );
    assert!(cancellation["exchange_id"].as_u64() > frame["exchange_id"].as_u64());
    reply(
        &mut peer,
        &cancellation,
        serde_json::json!({"target_exchange_id": frame["exchange_id"], "cancellation_requested": true}),
    )
    .await;
    assert_eq!(effects.settled.load(Ordering::Acquire), 0);
    reply(
        &mut peer,
        &frame,
        serde_json::json!({"native_settled": true}),
    )
    .await;
    next_clean(&channel, &mut peer).await;
    assert_eq!(effects.settled.load(Ordering::Acquire), 1);
    assert_eq!(effects.uncertain.load(Ordering::Acquire), 0);
    assert_eq!(stops.count.load(Ordering::Acquire), 0);
}

#[tokio::test]
async fn overlapping_wait_and_cancel_replies_are_correlated_independently() {
    let (channel, mut peer, stops) = channel();
    let status_effects = Arc::new(Effects::default());
    let cancel_effects = Arc::new(Effects::default());
    let status = call(&channel, "status", prepare(&status_effects));
    let status_frame = read_request(&mut peer).await;
    let cancel = call(&channel, "cancel", prepare(&cancel_effects));
    let cancel_frame = read_request(&mut peer).await;
    assert!(cancel_frame["exchange_id"].as_u64() > status_frame["exchange_id"].as_u64());
    reply(&mut peer, &cancel_frame, serde_json::json!({"cancel": 2})).await;
    assert_eq!(
        cancel.await.unwrap().unwrap(),
        serde_json::json!({"cancel": 2})
    );
    assert_eq!(status_effects.settled.load(Ordering::Acquire), 0);
    reply(&mut peer, &status_frame, serde_json::json!({"status": 1})).await;
    assert_eq!(
        status.await.unwrap().unwrap(),
        serde_json::json!({"status": 1})
    );
    next_clean(&channel, &mut peer).await;
    assert_eq!(stops.count.load(Ordering::Acquire), 0);
}

#[tokio::test]
async fn deferred_control_receives_writer_order_id_without_retargeting_original() {
    let (channel, mut peer, stops) = channel();
    let original = call(&channel, "hello", prepare(&Arc::new(Effects::default())));
    let original_frame = read_request(&mut peer).await;
    reply(&mut peer, &original_frame, serde_json::json!({})).await;
    original.await.unwrap().unwrap();

    let ordinary = call(&channel, "status", prepare(&Arc::new(Effects::default())));
    let ordinary_frame = read_request(&mut peer).await;
    // A cancellation control can be scheduled before an ordinary job but only
    // enqueue afterwards. IDs belong to serialized wire admission, not creation.
    let (reply_sender, result) = oneshot::channel();
    channel
        .queue
        .send(Job {
            operation: "cancel_exchange",
            payload: serde_json::json!({"target_exchange_id": original_frame["exchange_id"]}),
            prepare: Box::new(|| Ok(Box::new(ValueCustody))),
            reply: reply_sender,
            cancellation: Arc::new(ExchangeCancellation::default()),
        })
        .await
        .unwrap();
    let control_frame = read_request(&mut peer).await;
    assert_eq!(ordinary_frame["exchange_id"], 2);
    assert_eq!(control_frame["exchange_id"], 3);
    assert_eq!(control_frame["payload"]["target_exchange_id"], 1);
    reply(&mut peer, &control_frame, serde_json::json!({})).await;
    result.await.unwrap().unwrap();
    reply(&mut peer, &ordinary_frame, serde_json::json!({})).await;
    ordinary.await.unwrap().unwrap();
    assert_eq!(stops.count.load(Ordering::Acquire), 0);
}

#[tokio::test]
async fn valid_native_error_preserves_connection_reuse() {
    let (channel, mut peer, stops) = channel();
    let effects = Arc::new(Effects::default());
    let rejected = call(&channel, "use", prepare(&effects));
    let frame = read_request(&mut peer).await;
    send_bytes(
        &mut peer,
        &serde_json::to_vec(&serde_json::json!({
            "version":1, "exchange_id":frame["exchange_id"], "operation":"use",
            "error":{"code":"busy", "effect":"not_admitted"}
        }))
        .unwrap(),
    )
    .await;
    assert_eq!(rejected.await.unwrap(), Err(ChannelError::NativeRejected));
    next_clean(&channel, &mut peer).await;
    assert_eq!(effects.settled.load(Ordering::Acquire), 1);
    assert_eq!(effects.uncertain.load(Ordering::Acquire), 0);
    assert_eq!(stops.count.load(Ordering::Acquire), 0);
}

#[tokio::test]
async fn received_clean_load_error_does_not_emit_caller_loss_cancellation() {
    let (channel, mut peer, stops) = channel();
    let effects = Arc::new(Effects::default());
    let rejected = call(&channel, "load", prepare(&effects));
    let frame = read_request(&mut peer).await;
    send_bytes(
        &mut peer,
        &serde_json::to_vec(&serde_json::json!({
            "version":1, "exchange_id":frame["exchange_id"], "operation":"load",
            "error":{"code":"runtime_busy", "effect":"not_admitted"}
        }))
        .unwrap(),
    )
    .await;
    assert_eq!(rejected.await.unwrap(), Err(ChannelError::NativeRejected));
    next_clean(&channel, &mut peer).await;
    assert_eq!(effects.settled.load(Ordering::Acquire), 1);
    assert_eq!(stops.count.load(Ordering::Acquire), 0);
}

#[tokio::test]
async fn cancellation_control_requires_exact_target_and_positive_acknowledgement() {
    for result in [
        serde_json::json!({"target_exchange_id":99,"cancellation_requested":true}),
        serde_json::json!({"target_exchange_id":1,"cancellation_requested":false}),
        serde_json::json!({"target_exchange_id":1,"cancellation_requested":true,"extra":1}),
    ] {
        let (channel, mut peer, stops) = channel();
        let effects = Arc::new(Effects::default());
        let original = call(&channel, "load", prepare(&effects));
        read_request(&mut peer).await;
        original.abort();
        assert!(original.await.unwrap_err().is_cancelled());
        let cancellation = read_request(&mut peer).await;
        reply(&mut peer, &cancellation, result).await;
        stops.wait().await;
        assert_eq!(effects.settled.load(Ordering::Acquire), 0);
        assert_eq!(effects.uncertain.load(Ordering::Acquire), 1);
        assert_eq!(stops.count.load(Ordering::Acquire), 1);
    }
}

#[tokio::test]
async fn unknown_native_error_retains_custody_without_claiming_clean_settlement() {
    let (channel, mut peer, stops) = channel();
    let effects = Arc::new(Effects::default());
    let original = call(&channel, "use", prepare(&effects));
    let frame = read_request(&mut peer).await;
    send_bytes(
        &mut peer,
        &serde_json::to_vec(&serde_json::json!({
            "version":1, "exchange_id":frame["exchange_id"], "operation":"use",
            "error":{"code":"native_failure", "effect":"unknown"}
        }))
        .unwrap(),
    )
    .await;
    assert_eq!(original.await.unwrap(), Err(ChannelError::Unknown));
    assert_eq!(effects.settled.load(Ordering::Acquire), 0);
    assert_eq!(effects.uncertain.load(Ordering::Acquire), 1);
    stops.wait().await;
    assert_eq!(stops.count.load(Ordering::Acquire), 1);
    assert_eq!(
        channel
            .exchange::<Value>("status", serde_json::json!({}), prepare(&effects))
            .await,
        Err(ChannelError::NotAdmitted)
    );
}

#[tokio::test]
async fn incoherent_envelopes_quarantine_admitted_originals() {
    let malformed: &[&[u8]] = &[
        br#"{"version":1,"exchange_id":99,"operation":"status","result":{}}"#,
        br#"{"version":1,"exchange_id":1,"operation":"cancel","result":{}}"#,
        br#"{"version":1,"exchange_id":1,"exchange_id":1,"operation":"status","result":{}}"#,
        br#"{"version":1,"exchange_id":1,"operation":"status","result":{},"error":{"code":"busy","effect":"not_admitted"}}"#,
        br#"{"version":1,"exchange_id":1,"operation":"status","result":{},"error":null}"#,
        br#"{"version":1,"exchange_id":1,"operation":"status","result":null,"error":{"code":"busy","effect":"not_admitted"}}"#,
        br#"{"version":1,"exchange_id":1,"operation":"status","result":{"nested":{"same":1,"same":2}}}"#,
        br#"{"version":1,"exchange_id":1,"operation":"status","result":{},"extra":true}"#,
        br#"{"version":2,"exchange_id":1,"operation":"status","result":{}}"#,
        br#"{"version":1,"exchange_id":1,"operation":"status","error":{"code":"Busy","effect":"not_admitted"}}"#,
        br#"{"version":1,"exchange_id":1,"operation":"status","error":{"code":"busy","effect":"invented"}}"#,
        b"{",
    ];
    for bytes in malformed {
        let (channel, mut peer, stops) = channel();
        let effects = Arc::new(Effects::default());
        let original = call(&channel, "status", prepare(&effects));
        read_request(&mut peer).await;
        send_bytes(&mut peer, bytes).await;
        let outcome = original.await.unwrap();
        assert!(matches!(
            outcome,
            Err(ChannelError::Unknown | ChannelError::Incoherent)
        ));
        stops.wait().await;
        assert_eq!(effects.uncertain.load(Ordering::Acquire), 1);
        assert_eq!(effects.settled.load(Ordering::Acquire), 0);
        assert_eq!(stops.count.load(Ordering::Acquire), 1);
        assert_eq!(
            channel
                .exchange::<Value>("status", serde_json::json!({}), prepare(&effects))
                .await,
            Err(ChannelError::NotAdmitted)
        );
    }
}

#[tokio::test]
async fn duplicate_original_reply_quarantines_new_pending_exchange() {
    let (channel, mut peer, stops) = channel();
    let first_effects = Arc::new(Effects::default());
    let first = call(&channel, "status", prepare(&first_effects));
    let first_frame = read_request(&mut peer).await;
    reply(&mut peer, &first_frame, serde_json::json!({"first": true})).await;
    first.await.unwrap().unwrap();
    let second_effects = Arc::new(Effects::default());
    let second = call(&channel, "status", prepare(&second_effects));
    read_request(&mut peer).await;
    reply(
        &mut peer,
        &first_frame,
        serde_json::json!({"replayed": true}),
    )
    .await;
    assert_eq!(second.await.unwrap(), Err(ChannelError::Unknown));
    stops.wait().await;
    assert_eq!(first_effects.settled.load(Ordering::Acquire), 1);
    assert_eq!(second_effects.uncertain.load(Ordering::Acquire), 1);
    assert_eq!(stops.count.load(Ordering::Acquire), 1);
}

#[tokio::test]
async fn partial_reply_header_or_body_quarantines_instead_of_reusing_stream() {
    for partial in [vec![0, 0], vec![0, 0, 0, 12, b'{', b'"']] {
        let (channel, mut peer, stops) = channel();
        let effects = Arc::new(Effects::default());
        let original = call(&channel, "status", prepare(&effects));
        read_request(&mut peer).await;
        peer.write_all(&partial).await.unwrap();
        peer.shutdown().await.unwrap();
        assert_eq!(original.await.unwrap(), Err(ChannelError::Unknown));
        stops.wait().await;
        assert_eq!(effects.uncertain.load(Ordering::Acquire), 1);
        assert_eq!(stops.count.load(Ordering::Acquire), 1);
    }
}

#[tokio::test]
async fn zero_or_oversized_reply_is_rejected_before_body_allocation() {
    for size in [0, MAX_REPLY as u32 + 1] {
        let (channel, mut peer, stops) = channel();
        let effects = Arc::new(Effects::default());
        let original = call(&channel, "status", prepare(&effects));
        read_request(&mut peer).await;
        peer.write_all(&size.to_be_bytes()).await.unwrap();
        assert_eq!(original.await.unwrap(), Err(ChannelError::Unknown));
        stops.wait().await;
        assert_eq!(effects.uncertain.load(Ordering::Acquire), 1);
    }
}

#[tokio::test]
async fn oversized_request_has_no_custody_or_wire_effect_and_keeps_connection_clean() {
    let (channel, mut peer, stops) = channel();
    let effects = Arc::new(Effects::default());
    let outcome = channel
        .exchange::<Value>(
            "use",
            Value::String("x".repeat(MAX_REQUEST)),
            prepare(&effects),
        )
        .await;
    assert_eq!(outcome, Err(ChannelError::Limit));
    assert_eq!(effects.prepared.load(Ordering::Acquire), 0);
    assert_eq!(effects.admitted.load(Ordering::Acquire), 0);
    next_clean(&channel, &mut peer).await;
    assert_eq!(stops.count.load(Ordering::Acquire), 0);
}

struct PartialWriter {
    remaining: usize,
    written: Arc<Mutex<Vec<u8>>>,
}

struct GatedPartialWriter {
    partial: PartialWriter,
    entered: Option<oneshot::Sender<()>>,
    released: oneshot::Receiver<()>,
}

impl AsyncWrite for GatedPartialWriter {
    fn poll_write(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
        bytes: &[u8],
    ) -> Poll<io::Result<usize>> {
        if self.partial.remaining != 0 {
            return Pin::new(&mut self.partial).poll_write(context, bytes);
        }
        if let Some(entered) = self.entered.take() {
            let _ = entered.send(());
        }
        match std::future::Future::poll(Pin::new(&mut self.released), context) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(_) => Poll::Ready(Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "controlled failure after premature reply",
            ))),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

#[tokio::test]
async fn premature_correlated_reply_cannot_settle_custody_before_partial_write_fails() {
    let (reader, mut peer) = tokio::io::duplex(1024);
    let written = Arc::new(Mutex::new(Vec::new()));
    let (entered, wait_for_partial) = oneshot::channel();
    let (release, released) = oneshot::channel();
    let stops = Arc::new(Stops::default());
    let channel = PrivateAudioChannel::from_streams(
        GatedPartialWriter {
            partial: PartialWriter {
                remaining: 7,
                written: written.clone(),
            },
            entered: Some(entered),
            released,
        },
        reader,
        stops.callback(),
    );
    let effects = Arc::new(Effects::default());
    let original = call(&channel, "status", prepare(&effects));
    wait_for_partial.await.unwrap();
    send_bytes(
        &mut peer,
        br#"{"version":1,"exchange_id":1,"operation":"status","error":{"code":"busy","effect":"not_admitted"}}"#,
    )
    .await;
    // The reader has consumed the original envelope and is now waiting for its
    // writer's full-frame receipt. A JSON non-start claim cannot supply it.
    while !channel.shared.pending.lock().unwrap().is_empty() {
        tokio::task::yield_now().await;
    }
    assert_eq!(effects.settled.load(Ordering::Acquire), 0);
    assert_eq!(effects.uncertain.load(Ordering::Acquire), 0);
    assert_eq!(written.lock().unwrap().len(), 7);
    release.send(()).unwrap();
    assert_eq!(original.await.unwrap(), Err(ChannelError::Unknown));
    stops.wait().await;
    while effects.uncertain.load(Ordering::Acquire) == 0 {
        effects.changed.notified().await;
    }
    assert_eq!(effects.settled.load(Ordering::Acquire), 0);
    assert_eq!(effects.uncertain.load(Ordering::Acquire), 1);
    assert_eq!(stops.count.load(Ordering::Acquire), 1);
}

impl AsyncWrite for PartialWriter {
    fn poll_write(
        mut self: Pin<&mut Self>,
        _: &mut Context<'_>,
        bytes: &[u8],
    ) -> Poll<io::Result<usize>> {
        if self.remaining == 0 {
            return Poll::Ready(Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "controlled partial write",
            )));
        }
        let length = bytes.len().min(self.remaining);
        self.written
            .lock()
            .unwrap()
            .extend_from_slice(&bytes[..length]);
        self.remaining -= length;
        Poll::Ready(Ok(length))
    }

    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

#[tokio::test]
async fn partial_request_header_or_body_quarantines_admitted_custody() {
    for length in [2, 7] {
        let (reader, peer) = tokio::io::duplex(64);
        let written = Arc::new(Mutex::new(Vec::new()));
        let stops = Arc::new(Stops::default());
        let channel = PrivateAudioChannel::from_streams(
            PartialWriter {
                remaining: length,
                written: written.clone(),
            },
            reader,
            stops.callback(),
        );
        let effects = Arc::new(Effects::default());
        assert_eq!(
            call(&channel, "use", prepare(&effects)).await.unwrap(),
            Err(ChannelError::Unknown)
        );
        stops.wait().await;
        assert_eq!(written.lock().unwrap().len(), length);
        assert_eq!(effects.admitted.load(Ordering::Acquire), 1);
        assert_eq!(effects.uncertain.load(Ordering::Acquire), 1);
        assert_eq!(stops.count.load(Ordering::Acquire), 1);
        drop(peer);
    }
}

#[tokio::test]
async fn last_channel_owner_loss_quarantines_pending_custody_and_stops_once() {
    let (channel, mut peer, stops) = channel();
    let effects = Arc::new(Effects::default());
    let original = call(&channel, "status", prepare(&effects));
    read_request(&mut peer).await;
    original.abort();
    assert!(original.await.unwrap_err().is_cancelled());
    assert_eq!(effects.uncertain.load(Ordering::Acquire), 0);
    drop(channel);
    stops.wait().await;
    assert_eq!(effects.uncertain.load(Ordering::Acquire), 1);
    assert_eq!(stops.count.load(Ordering::Acquire), 1);
    drop(peer);
}

#[tokio::test]
async fn last_owner_loss_with_outstanding_cancel_does_not_keep_admission_alive() {
    let (channel, mut peer, stops) = channel();
    let effects = Arc::new(Effects::default());
    let original = call(&channel, "load", prepare(&effects));
    read_request(&mut peer).await;
    original.abort();
    assert!(original.await.unwrap_err().is_cancelled());
    let cancellation = read_request(&mut peer).await;
    assert_eq!(cancellation["operation"], "cancel_exchange");
    // Neither original load nor its cancellation control has replied. Their
    // reader-owned correlations retain custody, not an admission sender owner.
    drop(channel);
    stops.wait().await;
    assert_eq!(effects.settled.load(Ordering::Acquire), 0);
    assert_eq!(effects.uncertain.load(Ordering::Acquire), 1);
    assert_eq!(stops.count.load(Ordering::Acquire), 1);
    drop(peer);
}
