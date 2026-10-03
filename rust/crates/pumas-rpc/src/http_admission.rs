//! Admission for the local administrative HTTP listener.
//!
//! Loopback binding is not browser-origin authorization. These checks run before
//! CORS and dispatch. They preserve direct local clients without treating CORS
//! response headers as permission to perform a privileged operation.

use axum::extract::Request;
use axum::http::{header, uri::Authority, HeaderMap, HeaderValue, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use std::net::IpAddr;

pub(crate) async fn enforce_local_request(request: Request, next: Next) -> Response {
    if !is_allowed_request(request.headers()) {
        return (
            StatusCode::FORBIDDEN,
            "Local request origin is not permitted",
        )
            .into_response();
    }
    next.run(request).await
}

fn is_allowed_request(headers: &HeaderMap) -> bool {
    if headers.get_all(header::HOST).iter().count() != 1
        || !headers.get(header::HOST).is_some_and(is_allowed_host)
    {
        return false;
    }
    match headers.get_all(header::ORIGIN).iter().count() {
        0 => {
            // Cross-site browser GETs can omit Origin (for example an image
            // request). Native local clients do not send Fetch Metadata.
            headers.get_all("sec-fetch-site").iter().count() <= 1
                && headers.get("sec-fetch-site").is_none_or(|site| {
                    matches!(site.as_bytes(), b"none" | b"same-origin" | b"same-site")
                })
        }
        1 => headers.get(header::ORIGIN).is_some_and(is_allowed_origin),
        _ => false,
    }
}

fn is_allowed_host(value: &HeaderValue) -> bool {
    let Ok(value) = value.to_str() else {
        return false;
    };
    if value.contains('@') {
        return false;
    }
    let Ok(authority) = value.parse::<Authority>() else {
        return false;
    };
    if authority.port().is_some() && authority.port_u16().is_none() {
        return false;
    }
    let host = authority.host();
    let host = host
        .strip_prefix('[')
        .and_then(|host| host.strip_suffix(']'))
        .unwrap_or(host);
    host.eq_ignore_ascii_case("localhost")
        || host
            .parse::<IpAddr>()
            .is_ok_and(|address| address.is_loopback())
}

pub(crate) fn is_allowed_origin(value: &HeaderValue) -> bool {
    let Ok(value) = value.to_str() else {
        return false;
    };
    let Ok(origin) = url::Url::parse(value) else {
        return false;
    };
    if !matches!(origin.scheme(), "http" | "https")
        || !origin.username().is_empty()
        || origin.password().is_some()
        || origin.path() != "/"
        || origin.query().is_some()
        || origin.fragment().is_some()
    {
        return false;
    }
    match origin.host() {
        Some(url::Host::Domain("localhost")) => true,
        Some(url::Host::Ipv4(address)) => address.is_loopback(),
        Some(url::Host::Ipv6(address)) => address.is_loopback(),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn local_headers() -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(header::HOST, HeaderValue::from_static("127.0.0.1:9000"));
        headers
    }

    #[test]
    fn admission_preserves_native_and_loopback_browser_clients() {
        for host in [
            "127.0.0.1:9000",
            "localhost:9000",
            "LOCALHOST",
            "[::1]:9000",
        ] {
            let mut headers = local_headers();
            headers.insert(header::HOST, HeaderValue::from_str(host).unwrap());
            assert!(is_allowed_request(&headers), "{host}");
            for origin in [
                "http://localhost:5173",
                "http://127.0.0.1:9000",
                "https://[::1]",
            ] {
                headers.insert(header::ORIGIN, HeaderValue::from_str(origin).unwrap());
                assert!(is_allowed_request(&headers), "{host} {origin}");
            }
        }
    }

    #[test]
    fn admission_refuses_untrusted_or_non_origin_values() {
        for origin in [
            "https://untrusted.example",
            "null",
            "http://localhost.untrusted.example",
            "http://192.168.1.2",
            "http://user:secret@localhost",
            "http://localhost/path",
            "http://localhost?token=secret",
            "http://localhost#fragment",
            "file:///tmp/page",
        ] {
            let mut headers = local_headers();
            headers.insert(header::ORIGIN, HeaderValue::from_str(origin).unwrap());
            // A simple browser media type must never bypass source admission.
            headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("text/plain"));
            assert!(!is_allowed_request(&headers), "{origin}");
        }
    }

    #[test]
    fn admission_refuses_rebinding_and_ambiguous_headers() {
        for host in [
            "untrusted.example:9000",
            "localhost.untrusted.example",
            "0.0.0.0",
            "[::]",
            "user@localhost",
            "localhost:invalid",
            "localhost/path",
        ] {
            let mut headers = local_headers();
            headers.insert(header::HOST, HeaderValue::from_str(host).unwrap());
            assert!(!is_allowed_request(&headers), "{host}");
        }
        assert!(!is_allowed_request(&HeaderMap::new()));
        let mut headers = local_headers();
        headers.append(header::HOST, HeaderValue::from_static("localhost"));
        assert!(!is_allowed_request(&headers));
        let mut headers = local_headers();
        headers.append(header::ORIGIN, HeaderValue::from_static("http://localhost"));
        headers.append(header::ORIGIN, HeaderValue::from_static("http://localhost"));
        assert!(!is_allowed_request(&headers));
    }

    #[test]
    fn originless_browser_cross_site_requests_are_refused() {
        let mut headers = local_headers();
        for site in ["cross-site", "unknown"] {
            headers.insert("sec-fetch-site", HeaderValue::from_str(site).unwrap());
            assert!(!is_allowed_request(&headers));
        }
        for site in ["none", "same-origin", "same-site"] {
            headers.insert("sec-fetch-site", HeaderValue::from_str(site).unwrap());
            assert!(is_allowed_request(&headers));
        }
        headers.append("sec-fetch-site", HeaderValue::from_static("same-origin"));
        assert!(!is_allowed_request(&headers));
    }
}
