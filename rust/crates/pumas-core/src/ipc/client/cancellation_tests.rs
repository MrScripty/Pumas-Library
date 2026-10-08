use super::super::protocol::IpcError;
use super::*;
use std::future::Future;
use std::io;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use tokio::io::{
    duplex, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, DuplexStream, ReadBuf,
};
use tokio::net::TcpListener;
use tokio::sync::oneshot;

// Test watchdog only: production exchange drainage has no deadline.
async fn bounded<F: Future>(future: F) -> F::Output {
    tokio::time::timeout(std::time::Duration::from_secs(5), future)
        .await
        .expect("IPC fixture did not reach its next barrier")
}

async fn invoke(client: &IpcClient, label: &str) -> Result<serde_json::Value> {
    client
        .call(
            LocalIpcOperation::IntentEnsureModel,
            serde_json::json!({"label": label}),
        )
        .await
}

fn call(
    client: Arc<IpcClient>,
    label: &'static str,
) -> tokio::task::JoinHandle<Result<serde_json::Value>> {
    tokio::spawn(async move { invoke(&client, label).await })
}

async fn request<S: AsyncRead + Unpin>(stream: &mut S) -> IpcRequest {
    serde_json::from_slice(&read_frame(stream).await.unwrap().unwrap()).unwrap()
}

fn response_bytes(request: IpcRequest) -> Vec<u8> {
    serde_json::to_vec(&IpcResponse::success(
        request.id,
        request.params.unwrap()["label"].clone(),
    ))
    .unwrap()
}

async fn reply<S: AsyncWrite + Unpin>(stream: &mut S, request: IpcRequest) {
    write_frame(stream, &response_bytes(request)).await.unwrap();
}

fn test_client<S: AsyncRead + AsyncWrite + Unpin + Send + 'static>(stream: S) -> IpcClient {
    IpcClient::from_stream(stream, "127.0.0.1:12345".parse().unwrap(), 42)
}

fn assert_lost(result: Result<serde_json::Value>) {
    assert!(
        matches!(
            result,
            Err(PumasError::SharedInstanceLost {
                pid: 42,
                port: 12345
            })
        ),
        "{result:?}"
    );
}

async fn assert_no_request(peer: &mut DuplexStream) {
    let mut byte = [0];
    assert!(
        futures::poll!(Box::pin(peer.read(&mut byte))).is_pending(),
        "another request escaped while the prior reply was held"
    );
}

async fn assert_closed(peer: &mut DuplexStream) {
    let mut byte = [0];
    assert_eq!(
        bounded(peer.read(&mut byte)).await.unwrap(),
        0,
        "quarantined socket transmitted more request bytes"
    );
}

#[tokio::test]
async fn cancelled_after_write_drains_original_reply_before_two_clean_calls() {
    bounded(async {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let client = Arc::new(
            IpcClient::connect(listener.local_addr().unwrap(), 42)
                .await
                .unwrap(),
        );
        let (seen_tx, seen_rx) = oneshot::channel();
        let (release_tx, release_rx) = oneshot::channel();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let first = request(&mut stream).await;
            seen_tx.send(()).unwrap();
            release_rx.await.unwrap();
            reply(&mut stream, first).await;
            for _ in 0..2 {
                let next = request(&mut stream).await;
                reply(&mut stream, next).await;
            }
        });
        let first = call(client.clone(), "cancelled");
        seen_rx.await.unwrap();
        first.abort();
        assert!(first.await.unwrap_err().is_cancelled());
        release_tx.send(()).unwrap();
        let second = call(client.clone(), "second").await.unwrap();
        let third = call(client, "third").await.unwrap();
        server.await.unwrap();
        assert!(
            matches!(&second, Ok(value) if value == "second")
                && matches!(&third, Ok(value) if value == "third"),
            "following calls lost correlation: second={second:?}, third={third:?}"
        );
    })
    .await;
}

#[tokio::test]
async fn cancelled_queued_and_backpressured_calls_have_no_wire_effect() {
    bounded(async {
        let (stream, mut peer) = duplex(4096);
        let client = test_client(stream);
        let mut first = Box::pin(invoke(&client, "first"));
        assert!(futures::poll!(&mut first).is_pending());
        let first_request = request(&mut peer).await;
        let mut queued = Box::pin(invoke(&client, "cancelled-queued"));
        assert!(futures::poll!(&mut queued).is_pending());
        let mut backpressured = Box::pin(invoke(&client, "cancelled-backpressured"));
        assert!(futures::poll!(&mut backpressured).is_pending());
        drop(queued);
        drop(backpressured);
        let mut next = Box::pin(invoke(&client, "next"));
        assert!(futures::poll!(&mut next).is_pending());
        assert_no_request(&mut peer).await;
        reply(&mut peer, first_request).await;
        assert_eq!(first.await.unwrap(), "first");
        // This caller was waiting for channel capacity, so drive its send again.
        assert!(futures::poll!(&mut next).is_pending());
        let next_request = request(&mut peer).await;
        assert_eq!(next_request.params.as_ref().unwrap()["label"], "next");
        reply(&mut peer, next_request).await;
        assert_eq!(next.await.unwrap(), "next");
        assert_no_request(&mut peer).await;
    })
    .await;
}

struct ObservedStream {
    inner: DuplexStream,
    read_count: usize,
    write_count: usize,
    read_signal: Option<(usize, oneshot::Sender<()>)>,
    write_signal: Option<(usize, oneshot::Sender<()>)>,
    fail_write_after: Option<usize>,
}

impl ObservedStream {
    fn new(inner: DuplexStream) -> Self {
        Self {
            inner,
            read_count: 0,
            write_count: 0,
            read_signal: None,
            write_signal: None,
            fail_write_after: None,
        }
    }
}

fn signal_at(signal: &mut Option<(usize, oneshot::Sender<()>)>, count: usize) {
    if signal
        .as_ref()
        .is_some_and(|(threshold, _)| count >= *threshold)
    {
        let (_, sender) = signal.take().unwrap();
        let _ = sender.send(());
    }
}

impl AsyncRead for ObservedStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let before = buf.filled().len();
        let result = Pin::new(&mut self.inner).poll_read(cx, buf);
        self.read_count += buf.filled().len() - before;
        let count = self.read_count;
        signal_at(&mut self.read_signal, count);
        result
    }
}

impl AsyncWrite for ObservedStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let available = self
            .fail_write_after
            .map(|limit| limit - self.write_count)
            .unwrap_or(buf.len());
        if available == 0 {
            return Poll::Ready(Err(io::ErrorKind::BrokenPipe.into()));
        }
        let length = buf.len().min(available);
        let result = Pin::new(&mut self.inner).poll_write(cx, &buf[..length]);
        if let Poll::Ready(Ok(count)) = result {
            self.write_count += count;
            let count = self.write_count;
            signal_at(&mut self.write_signal, count);
        }
        result
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

#[tokio::test]
async fn cancellation_during_partial_write_finishes_original_exchange() {
    bounded(async {
        let (stream, mut peer) = duplex(2);
        let (written_tx, written_rx) = oneshot::channel();
        let mut stream = ObservedStream::new(stream);
        stream.write_signal = Some((2, written_tx));
        let client = test_client(stream);
        let mut first = Box::pin(invoke(&client, "cancelled"));
        assert!(futures::poll!(&mut first).is_pending());
        written_rx.await.unwrap(); // Exactly half of the frame prefix fits.
        drop(first);
        let exchange = async {
            let first_request = request(&mut peer).await;
            assert_eq!(first_request.params.as_ref().unwrap()["label"], "cancelled");
            reply(&mut peer, first_request).await;
            let next = request(&mut peer).await;
            reply(&mut peer, next).await;
        };
        let ((), next) = tokio::join!(exchange, invoke(&client, "next"));
        assert_eq!(next.unwrap(), "next");
    })
    .await;
}

#[tokio::test]
async fn cancelled_partial_reply_blocks_overlapping_calls_until_drain() {
    bounded(async {
        let (stream, mut peer) = duplex(4096);
        let (read_tx, read_rx) = oneshot::channel();
        let mut stream = ObservedStream::new(stream);
        stream.read_signal = Some((2, read_tx));
        let client = test_client(stream);
        let mut first = Box::pin(invoke(&client, "cancelled"));
        assert!(futures::poll!(&mut first).is_pending());
        let first_bytes = response_bytes(request(&mut peer).await);
        let prefix = (first_bytes.len() as u32).to_be_bytes();
        peer.write_all(&prefix[..2]).await.unwrap();
        read_rx.await.unwrap(); // Caller disappears after the parser consumed half a prefix.
        drop(first);
        let mut second = Box::pin(invoke(&client, "second"));
        let mut third = Box::pin(invoke(&client, "third"));
        assert!(futures::poll!(&mut second).is_pending());
        assert!(futures::poll!(&mut third).is_pending());
        assert_no_request(&mut peer).await;
        peer.write_all(&prefix[2..]).await.unwrap();
        peer.write_all(&first_bytes).await.unwrap();
        let second_request = request(&mut peer).await;
        assert_eq!(second_request.params.as_ref().unwrap()["label"], "second");
        // The second exchange has left the queue; drive the third caller's send.
        assert!(futures::poll!(&mut third).is_pending());
        let second_bytes = response_bytes(second_request);
        peer.write_all(&(second_bytes.len() as u32).to_be_bytes())
            .await
            .unwrap();
        peer.write_all(&second_bytes[..3]).await.unwrap();
        assert_no_request(&mut peer).await;
        peer.write_all(&second_bytes[3..]).await.unwrap();
        assert_eq!(second.await.unwrap(), "second");
        let third_request = request(&mut peer).await;
        assert_eq!(third_request.params.as_ref().unwrap()["label"], "third");
        reply(&mut peer, third_request).await;
        assert_eq!(third.await.unwrap(), "third");
    })
    .await;
}

#[tokio::test]
async fn partial_write_failure_quarantines_without_retry() {
    bounded(async {
        for accepted_bytes in [2, 7] {
            // Partial prefix, then partial payload.
            let (stream, mut peer) = duplex(4096);
            let mut stream = ObservedStream::new(stream);
            stream.fail_write_after = Some(accepted_bytes);
            let client = test_client(stream);
            assert_lost(invoke(&client, "uncertain").await);
            assert_lost(invoke(&client, "must-not-write").await);
            let mut bytes = Vec::new();
            peer.read_to_end(&mut bytes).await.unwrap();
            assert_eq!(bytes.len(), accepted_bytes);
        }
    })
    .await;
}

#[tokio::test]
async fn eof_partial_and_oversized_reads_quarantine_queued_and_future_calls() {
    bounded(async {
        let oversized = ((RegistryConfig::MAX_IPC_MESSAGE_SIZE + 1) as u32)
            .to_be_bytes()
            .to_vec();
        for bytes in [Vec::new(), vec![0, 0], vec![0, 0, 0, 8, b'{'], oversized] {
            let (stream, mut peer) = duplex(4096);
            let client = test_client(stream);
            let mut first = Box::pin(invoke(&client, "uncertain"));
            assert!(futures::poll!(&mut first).is_pending());
            request(&mut peer).await;
            let mut queued = Box::pin(invoke(&client, "must-not-write"));
            assert!(futures::poll!(&mut queued).is_pending());
            peer.write_all(&bytes).await.unwrap();
            peer.shutdown().await.unwrap();
            assert_lost(first.await);
            assert_lost(queued.await);
            assert_lost(invoke(&client, "later").await);
            assert_closed(&mut peer).await;
        }
    })
    .await;
}

#[tokio::test]
async fn malformed_envelopes_and_wrong_original_ids_quarantine() {
    bounded(async {
        let replies: Vec<&[u8]> = vec![
            b"not json",
            br#"{"jsonrpc":"2.0","id":2,"result":"wrong"}"#,
            br#"{"jsonrpc":"2.0","id":"1","result":"wrong"}"#,
            br#"{"jsonrpc":"2.0","id":null,"result":"wrong"}"#,
            br#"{"jsonrpc":"2.0","result":"missing-id"}"#,
            br#"{"jsonrpc":"1.0","id":1,"result":"wrong-version"}"#,
            br#"{"jsonrpc":"2.0","id":1}"#,
            br#"{"jsonrpc":"2.0","id":1,"result":"both","error":{"code":-32603,"message":"error"}}"#,
            br#"{"jsonrpc":"2.0","id":1,"result":null,"error":{"code":-32603,"message":"error"}}"#,
            br#"{"jsonrpc":"2.0","id":1,"result":"success","error":null}"#,
            br#"{"jsonrpc":"2.0","id":1,"result":"success","extra":true}"#,
            br#"{"jsonrpc":"2.0","id":1,"id":1,"result":"duplicate"}"#,
            br#"{"jsonrpc":"2.0","id":1,"error":{"code":"bad","message":"error"}}"#,
        ];
        for bytes in replies {
            let (stream, mut peer) = duplex(4096);
            let client = test_client(stream);
            let mut first = Box::pin(invoke(&client, "first"));
            assert!(futures::poll!(&mut first).is_pending());
            let original = request(&mut peer).await;
            assert_eq!(original.id, Some(serde_json::json!(1)));
            drop(first); // Invalid orphan replies must still be validated.
            let mut queued = Box::pin(invoke(&client, "must-not-write"));
            assert!(futures::poll!(&mut queued).is_pending());
            write_frame(&mut peer, bytes).await.unwrap();
            assert_lost(queued.await);
            assert_lost(invoke(&client, "later").await);
            assert_closed(&mut peer).await;
        }
    }).await;
}

#[tokio::test]
async fn invalid_reply_reports_original_diagnostic_then_connection_loss() {
    bounded(async {
        for (bytes, expected) in [
            (b"not json".as_slice(), "json"),
            (
                br#"{"jsonrpc":"2.0","id":2,"result":true}"#.as_slice(),
                "correlation",
            ),
            (br#"{"jsonrpc":"2.0","id":1}"#.as_slice(), "outcome"),
        ] {
            let (stream, mut peer) = duplex(4096);
            let client = test_client(stream);
            let mut first = Box::pin(invoke(&client, "first"));
            assert!(futures::poll!(&mut first).is_pending());
            request(&mut peer).await;
            write_frame(&mut peer, bytes).await.unwrap();
            let error = first.await.unwrap_err();
            match expected {
                "json" => assert!(matches!(error, PumasError::Json { .. })),
                name => assert!(
                    matches!(error, PumasError::InvalidParams { message } if message.contains(name))
                ),
            }
            assert_lost(invoke(&client, "later").await);
            assert_closed(&mut peer).await;
        }
    })
    .await;
}

#[tokio::test]
async fn stale_extra_response_is_never_accepted_as_the_next_reply() {
    bounded(async {
        let (stream, mut peer) = duplex(4096);
        let client = test_client(stream);
        let mut first = Box::pin(invoke(&client, "first"));
        assert!(futures::poll!(&mut first).is_pending());
        let bytes = response_bytes(request(&mut peer).await);
        write_frame(&mut peer, &bytes).await.unwrap();
        write_frame(&mut peer, &bytes).await.unwrap();
        assert_eq!(first.await.unwrap(), "first");
        assert!(matches!(invoke(&client, "second").await, Err(PumasError::InvalidParams { message }) if message.contains("correlation")));
        let second = request(&mut peer).await;
        assert_eq!(second.params.unwrap()["label"], "second");
        assert_lost(invoke(&client, "third").await);
        assert_closed(&mut peer).await;
    }).await;
}

#[tokio::test]
async fn valid_rpc_errors_and_null_success_allow_clean_reuse() {
    bounded(async {
        let (stream, mut peer) = duplex(4096);
        let client = test_client(stream);
        for code in [-32602, -32603, -32004] {
            for cancelled in [false, true] {
                let mut error_call = Box::pin(invoke(&client, "rpc-error"));
                assert!(futures::poll!(&mut error_call).is_pending());
                let original = request(&mut peer).await;
                let bytes = serde_json::to_vec(&IpcResponse::error(original.id, IpcError { code, message: "valid rpc error".into(), data: None })).unwrap();
                if cancelled {
                    drop(error_call);
                } else {
                    write_frame(&mut peer, &bytes).await.unwrap();
                    let result = error_call.await;
                    if code == -32602 {
                        assert!(matches!(result, Err(PumasError::InvalidParams { message }) if message == "valid rpc error"));
                    } else {
                        assert!(matches!(result, Err(PumasError::Other(message)) if message == "valid rpc error"));
                    }
                    continue;
                }
                write_frame(&mut peer, &bytes).await.unwrap();
            }
        }
        let mut null_call = Box::pin(invoke(&client, "null"));
        assert!(futures::poll!(&mut null_call).is_pending());
        let original = request(&mut peer).await;
        let null = IpcResponse::success(original.id, serde_json::Value::Null);
        write_frame(&mut peer, &serde_json::to_vec(&null).unwrap()).await.unwrap();
        assert_eq!(null_call.await.unwrap(), serde_json::Value::Null);
        let mut clean = Box::pin(invoke(&client, "clean"));
        assert!(futures::poll!(&mut clean).is_pending());
        let original = request(&mut peer).await;
        reply(&mut peer, original).await;
        assert_eq!(clean.await.unwrap(), "clean");
    }).await;
}

#[tokio::test]
async fn last_owner_disposal_closes_hung_exchange_without_waiting_for_reply() {
    bounded(async {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let client = Arc::new(
            IpcClient::connect(listener.local_addr().unwrap(), 42)
                .await
                .unwrap(),
        );
        let (mut peer, _) = listener.accept().await.unwrap();
        let first = call(client.clone(), "hung");
        request(&mut peer).await;
        first.abort();
        assert!(first.await.unwrap_err().is_cancelled());
        // Caller loss alone retains the socket while the explicit owner lives.
        let mut byte = [0];
        assert!(futures::poll!(Box::pin(peer.read(&mut byte))).is_pending());
        drop(client);
        assert_eq!(peer.read(&mut byte).await.unwrap(), 0);
    })
    .await;
}
