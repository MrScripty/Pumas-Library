use std::time::Duration;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    task::JoinHandle,
};

pub(super) fn head(version: Option<&str>, etag: Option<&str>) -> String {
    format!("HTTP/1.1 200 OK\r\nContent-Length: 8\r\nLast-Modified: Wed, 01 Jan 2025 00:00:00 GMT\r\n{}{}Connection: close\r\n\r\n",
        version.map(|v| format!("x-amz-version-id: {v}\r\n")).unwrap_or_default(),
        etag.map(|v| format!("ETag: {v}\r\n")).unwrap_or_default())
}

pub(super) fn range_response(
    version: &str,
    etag: &str,
    total: u64,
    range: &str,
    body: &str,
) -> String {
    format!("HTTP/1.1 206 Partial Content\r\nContent-Length: {}\r\nContent-Range: bytes {range}/{total}\r\nLast-Modified: Wed, 01 Jan 2025 00:00:00 GMT\r\nx-amz-version-id: {version}\r\nETag: {etag}\r\nConnection: close\r\n\r\n{body}", body.len())
}

pub(super) async fn request(socket: &mut TcpStream) -> String {
    let mut data = Vec::new();
    while !data.ends_with(b"\r\n\r\n") {
        assert!(data.len() < 16 * 1024, "fixture request too large");
        data.push(socket.read_u8().await.unwrap());
    }
    String::from_utf8(data).unwrap()
}

pub(super) struct Fixture {
    pub(super) endpoint: String,
    pub(super) task: JoinHandle<Vec<String>>,
    pub(super) stop: Option<tokio::sync::oneshot::Sender<()>>,
}

impl Fixture {
    pub(super) async fn serve(responses: Vec<String>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let (stop_tx, mut stop_rx) = tokio::sync::oneshot::channel();
        let task = tokio::spawn(async move {
            let mut requests = Vec::new();
            for response in responses {
                let (mut socket, _) = listener.accept().await.unwrap();
                requests.push(request(&mut socket).await);
                socket.write_all(response.as_bytes()).await.unwrap();
                socket.shutdown().await.unwrap();
            }
            // Keep the source reachable until the operation returns, so a retry
            // or followed redirect is observable rather than hidden by refusal.
            loop {
                tokio::select! {
                    _ = &mut stop_rx => break,
                    accepted = listener.accept() => {
                        let (mut socket, _) = accepted.unwrap();
                        requests.push(request(&mut socket).await);
                        socket.write_all(b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await.unwrap();
                        socket.shutdown().await.unwrap();
                    }
                }
            }
            requests
        });
        Self {
            endpoint,
            task,
            stop: Some(stop_tx),
        }
    }

    pub(super) async fn finish(mut self) -> Vec<String> {
        if let Some(stop) = self.stop.take() {
            stop.send(()).unwrap();
        }
        tokio::time::timeout(Duration::from_secs(5), &mut self.task)
            .await
            .expect("fixture did not finish")
            .expect("fixture failed")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.task.abort();
    }
}
