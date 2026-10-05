use std::{sync::Arc, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    task::JoinHandle,
};

pub(super) struct Fixture {
    pub endpoint: String,
    pub address: std::net::SocketAddr,
    pub certificate: Option<reqwest::Certificate>,
    stop: Option<tokio::sync::oneshot::Sender<()>>,
    task: JoinHandle<Vec<String>>,
}
impl Fixture {
    pub async fn serve(responses: Vec<String>, tls: bool) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let endpoint = format!("{}://{address}", if tls { "https" } else { "http" });
        let (certificate, acceptor) = if tls {
            let rcgen::CertifiedKey { cert, signing_key } =
                rcgen::generate_simple_self_signed(vec![
                    "127.0.0.1".into(),
                    "fixture-bucket.localhost".into(),
                ])
                .unwrap();
            let certificate = reqwest::Certificate::from_der(cert.der().as_ref()).unwrap();
            let key = tokio_rustls::rustls::pki_types::PrivatePkcs8KeyDer::from(
                signing_key.serialize_der(),
            );
            let config = tokio_rustls::rustls::ServerConfig::builder_with_provider(Arc::new(
                tokio_rustls::rustls::crypto::ring::default_provider(),
            ))
            .with_safe_default_protocol_versions()
            .unwrap()
            .with_no_client_auth()
            .with_single_cert(vec![cert.der().clone()], key.into())
            .unwrap();
            (
                Some(certificate),
                Some(tokio_rustls::TlsAcceptor::from(Arc::new(config))),
            )
        } else {
            (None, None)
        };
        let (stop, mut stopped) = tokio::sync::oneshot::channel();
        let task = tokio::spawn(async move {
            let mut responses = responses.into_iter();
            let mut requests = Vec::new();
            loop {
                let accepted = tokio::select! { _ = &mut stopped => break, accepted = listener.accept() => accepted.unwrap() };
                let socket = accepted.0;
                let mut socket: Box<dyn AsyncSocket> = if let Some(acceptor) = &acceptor {
                    match tokio::time::timeout(Duration::from_secs(5), acceptor.accept(socket))
                        .await
                    {
                        Ok(Ok(socket)) => Box::new(socket),
                        _ => continue,
                    }
                } else {
                    Box::new(socket)
                };
                let mut data = Vec::new();
                let read = tokio::time::timeout(Duration::from_secs(5), async {
                    while !data.ends_with(b"\r\n\r\n") {
                        if data.len() > 16 * 1024 {
                            return false;
                        }
                        match socket.read_u8().await {
                            Ok(byte) => data.push(byte),
                            Err(_) => return false,
                        }
                    }
                    true
                })
                .await
                .unwrap_or(false);
                if !read {
                    continue;
                }
                requests.push(String::from_utf8(data).unwrap());
                let response = responses.next().unwrap_or_else(|| "HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".into());
                let _ = socket.write_all(response.as_bytes()).await;
                let _ = socket.shutdown().await;
            }
            requests
        });
        Self {
            endpoint,
            address,
            certificate,
            stop: Some(stop),
            task,
        }
    }
    pub async fn finish(mut self) -> Vec<String> {
        self.stop.take().unwrap().send(()).unwrap();
        tokio::time::timeout(Duration::from_secs(6), &mut self.task)
            .await
            .expect("fixture did not stop")
            .unwrap()
    }
}
trait AsyncSocket: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send {}
impl<T: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send> AsyncSocket for T {}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.task.abort();
    }
}

pub(super) fn head(version: &str, etag: &str) -> String {
    format!("HTTP/1.1 200 OK\r\nContent-Length: 8\r\nETag: {etag}\r\nx-amz-version-id: {version}\r\nConnection: close\r\n\r\n")
}
pub(super) fn range(
    version: &str,
    etag: &str,
    status: u16,
    range: Option<&str>,
    body: &str,
) -> String {
    format!("HTTP/1.1 {status} Fixture\r\nContent-Length: {}\r\nETag: {etag}\r\nx-amz-version-id: {version}\r\n{}Connection: close\r\n\r\n{body}", body.len(), range.map(|v| format!("Content-Range: {v}\r\n")).unwrap_or_default())
}
pub(super) fn xml(body: &str) -> String {
    format!("HTTP/1.1 200 OK\r\nContent-Type: application/xml\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len())
}
