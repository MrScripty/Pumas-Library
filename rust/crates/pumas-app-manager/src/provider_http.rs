//! HTTP policy shared by provider clients (Ollama, and any other client that
//! may reach either a local runtime or a configured remote provider).
//!
//! Remote runtime profiles are a supported configuration, so these clients
//! keep honouring the user's `HTTP_PROXY`, `HTTPS_PROXY` and `ALL_PROXY` for
//! remote hosts. Loopback hosts always bypass every proxy: prompts, models and
//! embeddings destined for a provider on this machine must not be routed
//! through a proxy the user configured for the internet.

use pumas_library::{PumasError, Result};
use reqwest::{ClientBuilder, NoProxy, Proxy, Url};
use tracing::warn;

/// Hosts that never go through a proxy: the whole IPv4 loopback block, IPv6
/// loopback, and `localhost` (with its subdomains).
pub(crate) const LOOPBACK_NO_PROXY: &str = "127.0.0.0/8,::1,localhost";

/// Apply the loopback proxy bypass to `builder`, keeping the environment's
/// proxies for every other host. `NO_PROXY` entries are kept as well.
pub(crate) fn bypass_proxy_for_loopback(builder: ClientBuilder) -> ClientBuilder {
    bypass_proxy_for_loopback_with(builder, |name| std::env::var(name).ok())
}

fn bypass_proxy_for_loopback_with(
    mut builder: ClientBuilder,
    env: impl Fn(&str) -> Option<String>,
) -> ClientBuilder {
    let first = |names: [&str; 2]| {
        names
            .into_iter()
            .find_map(|name| env(name).filter(|value| !value.trim().is_empty()))
    };
    let no_proxy = match first(["NO_PROXY", "no_proxy"]) {
        Some(list) => format!("{LOOPBACK_NO_PROXY},{list}"),
        None => LOOPBACK_NO_PROXY.to_owned(),
    };
    type ProxyFor = fn(&str) -> reqwest::Result<Proxy>;
    let schemes: [([&str; 2], ProxyFor); 3] = [
        (["HTTP_PROXY", "http_proxy"], |url| Proxy::http(url)),
        (["HTTPS_PROXY", "https_proxy"], |url| Proxy::https(url)),
        (["ALL_PROXY", "all_proxy"], |url| Proxy::all(url)),
    ];
    for (names, proxy_for) in schemes {
        let Some(url) = first(names) else {
            continue;
        };
        // Adding any explicit proxy disables reqwest's automatic system proxy,
        // so each environment proxy is re-added here with the bypass list.
        match proxy_for(&url) {
            Ok(proxy) => {
                builder = builder.proxy(proxy.no_proxy(NoProxy::from_string(&no_proxy)));
            }
            Err(error) => warn!("Ignoring invalid {} proxy setting: {error}", names[0]),
        }
    }
    builder
}

/// Whether `url` names a loopback host: an IPv4 or IPv6 loopback address or
/// `localhost`.
pub(crate) fn is_loopback_url(url: &Url) -> bool {
    let Some(host) = url.host_str() else {
        return false;
    };
    match host.trim_matches(['[', ']']).parse::<std::net::IpAddr>() {
        Ok(address) => address.is_loopback(),
        Err(_) => host.eq_ignore_ascii_case("localhost"),
    }
}

/// Parse a provider base URL, requiring http(s) and a host.
pub(crate) fn parse_provider_url(base_url: &str, provider: &str) -> Result<Url> {
    let url = Url::parse(base_url).map_err(|error| PumasError::Config {
        message: format!("{provider} base URL is invalid: {error}"),
    })?;
    if !matches!(url.scheme(), "http" | "https") || url.host().is_none() {
        return Err(PumasError::Config {
            message: format!("{provider} base URL must be an http or https URL with a host"),
        });
    }
    Ok(url)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    /// A listener that records the first request line of each connection and
    /// answers with an empty 200.
    async fn recording_listener() -> (String, Arc<Mutex<Vec<String>>>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap().to_string();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let recorded = seen.clone();
        tokio::spawn(async move {
            while let Ok((mut stream, _)) = listener.accept().await {
                let mut request = [0; 2048];
                let read = stream.read(&mut request).await.unwrap_or(0);
                recorded.lock().unwrap().push(
                    String::from_utf8_lossy(&request[..read])
                        .lines()
                        .next()
                        .unwrap_or_default()
                        .to_owned(),
                );
                let _ = stream
                    .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                    .await;
            }
        });
        (address, seen)
    }

    #[tokio::test]
    async fn loopback_hosts_bypass_an_environment_proxy_and_remote_hosts_keep_it() {
        let (proxy, proxied) = recording_listener().await;
        let (target, direct) = recording_listener().await;
        let env: HashMap<&str, String> = [
            ("HTTP_PROXY", format!("http://{proxy}")),
            ("HTTPS_PROXY", format!("http://{proxy}")),
        ]
        .into();
        let client = bypass_proxy_for_loopback_with(reqwest::Client::builder(), |name| {
            env.get(name).cloned()
        })
        .build()
        .unwrap();

        client
            .get(format!("http://{target}/api/tags"))
            .send()
            .await
            .unwrap();
        assert_eq!(
            direct.lock().unwrap().as_slice(),
            ["GET /api/tags HTTP/1.1"]
        );
        assert!(proxied.lock().unwrap().is_empty());

        // A remote provider still goes through the user's proxy. An HTTPS
        // remote is tunnelled, so the proxy's CONNECT line proves the route;
        // the tunnelled TLS handshake then fails against the recording
        // listener, which is expected and not asserted.
        let _ = client
            .get("https://remote-provider.invalid/api/tags")
            .send()
            .await;
        assert_eq!(
            proxied.lock().unwrap().as_slice(),
            ["CONNECT remote-provider.invalid:443 HTTP/1.1"]
        );
    }

    #[test]
    fn loopback_urls_are_recognised() {
        for url in [
            "http://127.0.0.1:11434",
            "http://127.4.5.6",
            "http://[::1]:11434",
            "http://localhost:11434",
        ] {
            assert!(is_loopback_url(&Url::parse(url).unwrap()), "{url}");
        }
        for url in ["http://192.168.1.10:11434", "https://ollama.example.com"] {
            assert!(!is_loopback_url(&Url::parse(url).unwrap()), "{url}");
        }
    }
}
