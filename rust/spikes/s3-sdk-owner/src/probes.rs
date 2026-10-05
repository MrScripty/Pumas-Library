use super::{
    client::{self, Addressing, ACCESS, BUCKET, SECRET, TOKEN, VERSION},
    fixture::{self, Fixture},
};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    io::Write,
    sync::{Arc, Mutex},
};
use tracing::instrument::WithSubscriber;

fn reader(
    fixture: &Fixture,
    addressing: Addressing,
    auth: Option<Option<&str>>,
    limit: usize,
) -> aws_sdk_s3::Client {
    client::build(
        &fixture.endpoint,
        addressing,
        auth,
        fixture.certificate.clone(),
        None,
        limit,
        true,
    )
    .unwrap()
}

// Same test-only RFC 2104 oracle used by the frozen reader qualification. The
// maintained SDK owns every production signature; this checks captured bytes.
fn hmac(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut block = [0; 64];
    let hashed;
    let key = if key.len() > block.len() {
        hashed = Sha256::digest(key);
        &hashed[..]
    } else {
        key
    };
    block[..key.len()].copy_from_slice(key);
    let inner: Vec<_> = block
        .iter()
        .map(|b| b ^ 0x36)
        .chain(data.iter().copied())
        .collect();
    let outer: Vec<_> = block
        .iter()
        .map(|b| b ^ 0x5c)
        .chain(Sha256::digest(inner))
        .collect();
    Sha256::digest(outer).to_vec()
}
fn valid_signature(request: &str, secret: &str) -> bool {
    let mut lines = request.split("\r\n");
    let mut first = lines.next().unwrap().split_whitespace();
    let method = first.next().unwrap();
    let (path, query) = first.next().unwrap().split_once('?').unwrap();
    let headers: BTreeMap<_, _> = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(key, value)| (key.to_ascii_lowercase(), value.trim().to_owned()))
        .collect();
    let fields: Vec<_> = headers["authorization"]
        .strip_prefix("AWS4-HMAC-SHA256 ")
        .unwrap()
        .split(", ")
        .collect();
    let (access, scope) = fields[0]
        .strip_prefix("Credential=")
        .unwrap()
        .split_once('/')
        .unwrap();
    assert_eq!(access, ACCESS);
    let signed = fields[1].strip_prefix("SignedHeaders=").unwrap();
    let canonical_headers: String = signed
        .split(';')
        .map(|name| format!("{name}:{}\n", headers[name]))
        .collect();
    // SigV4 sorts encoded query pairs; the SDK need not send them in that order.
    let mut query_parts: Vec<_> = query.split('&').collect();
    query_parts.sort_unstable();
    let query = query_parts.join("&");
    let canonical = format!(
        "{method}\n{path}\n{query}\n{canonical_headers}\n{signed}\n{}",
        headers["x-amz-content-sha256"]
    );
    let to_sign = format!(
        "AWS4-HMAC-SHA256\n{}\n{scope}\n{}",
        headers["x-amz-date"],
        hex::encode(Sha256::digest(canonical))
    );
    let scope_parts: Vec<_> = scope.split('/').collect();
    assert_eq!(&scope_parts[1..], &["fixture-region", "s3", "aws4_request"]);
    let date_key = hmac(
        format!("AWS4{secret}").as_bytes(),
        scope_parts[0].as_bytes(),
    );
    let region_key = hmac(&date_key, b"fixture-region");
    let service_key = hmac(&region_key, b"s3");
    let signing_key = hmac(&service_key, b"aws4_request");
    hex::encode(hmac(&signing_key, to_sign.as_bytes()))
        == fields[2].strip_prefix("Signature=").unwrap()
}
#[test]
fn signature_oracle_has_independent_rfc4231_anchor() {
    assert_eq!(
        hex::encode(hmac(&[0x0b; 20], b"Hi There")),
        "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7"
    );
}

#[tokio::test]
async fn anonymous_and_signed_version_ranges_preserve_evidence_and_addressing() {
    for addressing in [Addressing::Path, Addressing::VirtualHosted] {
        let mut identities = Vec::new();
        for auth in [None, Some(None), Some(Some(TOKEN))] {
            let fixture = Fixture::serve(
                vec![
                    fixture::head(VERSION, "\"selected\""),
                    fixture::range(VERSION, "\"selected\"", 206, Some("bytes 2-5/8"), "cdef"),
                ],
                true,
            )
            .await;
            let sdk = reader(&fixture, addressing, auth, 4096);
            let (size, etag, version) = client::select(&sdk).await.unwrap();
            assert_eq!(client::range(&sdk, size, &etag).await.unwrap(), b"cdef");
            identities.push((size, etag, version));
            let requests = fixture.finish().await;
            assert_eq!(requests.len(), 2);
            for request in &requests {
                let prefix = match addressing {
                    Addressing::Path => "/fixture-bucket/models/",
                    Addressing::VirtualHosted => "/models/",
                };
                assert!(request.lines().next().unwrap().contains(prefix));
                assert!(request.contains("a%20b%25%3F.bin"));
                assert!(request.contains("versionId=v%2B1%2F%3D"));
                assert_eq!(
                    request.to_ascii_lowercase().contains("authorization:"),
                    auth.is_some()
                );
                assert_eq!(
                    request.contains(&format!("x-amz-security-token: {TOKEN}")),
                    auth == Some(Some(TOKEN))
                );
                if auth.is_some() {
                    assert!(valid_signature(request, SECRET));
                    assert!(!valid_signature(request, "wrong synthetic secret"));
                }
            }
            assert!(requests[1].contains("range: bytes=2-5"));
            assert!(requests[1].contains("if-match: \"selected\""));
        }
        assert!(identities.windows(2).all(|pair| pair[0] == pair[1]));
    }
}

#[tokio::test]
async fn changed_versions_validators_ranges_and_body_lengths_are_refused() {
    for response in [
        fixture::range("changed", "\"selected\"", 206, Some("bytes 2-5/8"), "cdef"),
        fixture::range(VERSION, "\"changed\"", 206, Some("bytes 2-5/8"), "cdef"),
        fixture::range(VERSION, "\"selected\"", 200, None, "abcdefgh"),
        fixture::range(VERSION, "\"selected\"", 206, Some("bytes 1-4/8"), "cdef"),
        fixture::range(VERSION, "\"selected\"", 206, Some("bytes 2-5/9"), "cdef"),
        fixture::range(VERSION, "\"selected\"", 206, Some("bytes 2-5/8"), "cde"),
        fixture::range(VERSION, "\"selected\"", 206, Some("bytes 2-5/8"), "cdefg"),
        fixture::range(
            VERSION,
            "\"selected\"",
            206,
            Some("bytes 2-5/8\r\nContent-Range: bytes 2-5/8"),
            "cdef",
        ),
    ] {
        let fixture = Fixture::serve(vec![response], false).await;
        let sdk = reader(&fixture, Addressing::Path, Some(None), 4096);
        assert!(client::range(&sdk, 8, "\"selected\"").await.is_err());
        assert_eq!(fixture.finish().await.len(), 1);
    }
    for version in ["changed", "null", ""] {
        let fixture = Fixture::serve(vec![fixture::head(version, "\"selected\"")], false).await;
        let sdk = reader(&fixture, Addressing::Path, None, 4096);
        assert!(client::select(&sdk).await.is_err());
        assert_eq!(fixture.finish().await.len(), 1);
    }
}

#[tokio::test]
async fn chunked_ranges_verify_actual_body_counts_without_requiring_content_length() {
    for (body, valid) in [("cdef", true), ("cde", false), ("cdefg", false)] {
        let response = format!("HTTP/1.1 206 Partial Content\r\nTransfer-Encoding: chunked\r\nContent-Range: bytes 2-5/8\r\nx-amz-version-id: {VERSION}\r\nETag: \"selected\"\r\nConnection: close\r\n\r\n{:x}\r\n{body}\r\n0\r\n\r\n", body.len());
        let fixture = Fixture::serve(vec![response], false).await;
        let sdk = reader(&fixture, Addressing::Path, Some(Some(TOKEN)), 4096);
        let result = client::range(&sdk, 8, "\"selected\"").await;
        assert_eq!(result.is_ok(), valid);
        if valid {
            assert_eq!(result.unwrap(), b"cdef");
        }
        assert_eq!(fixture.finish().await.len(), 1);
    }
    let response = fixture::range(VERSION, "\"selected\"", 206, Some("bytes 2-5/8"), "cde")
        .replace("Content-Length: 3", "Content-Length: 4");
    let fixture = Fixture::serve(vec![response], false).await;
    let sdk = reader(&fixture, Addressing::Path, None, 4096);
    assert!(client::range(&sdk, 8, "\"selected\"").await.is_err());
    assert_eq!(fixture.finish().await.len(), 1);
}

#[tokio::test]
async fn production_https_and_certificate_validation_are_retained() {
    for endpoint in [
        "http://127.0.0.1:9",
        "http://localhost:9",
        "https://user:password@127.0.0.1:9",
        "https://127.0.0.1:9/?token=x",
    ] {
        assert!(client::build(
            endpoint,
            Addressing::Path,
            Some(None),
            None,
            None,
            4096,
            false
        )
        .is_err());
    }
    let fixture = Fixture::serve(vec![fixture::head(VERSION, "\"selected\"")], true).await;
    let sdk = client::build(
        &fixture.endpoint,
        Addressing::Path,
        Some(None),
        None,
        None,
        4096,
        false,
    )
    .unwrap();
    assert!(client::select(&sdk).await.is_err());
    assert!(fixture.finish().await.is_empty());
}

#[tokio::test]
async fn virtual_endpoint_keeps_exact_host_and_bucket_authority() {
    let fixture = Fixture::serve(vec![fixture::head(VERSION, "\"selected\"")], true).await;
    let endpoint = fixture
        .endpoint
        .replace("127.0.0.1", "fixture-bucket.localhost");
    let sdk = client::build(
        &endpoint,
        Addressing::VirtualHosted,
        Some(Some(TOKEN)),
        fixture.certificate.clone(),
        Some(fixture.address),
        4096,
        false,
    )
    .unwrap();
    assert!(format!("{sdk:?}").contains("Client"));
    assert!(!format!("{sdk:?}").contains(ACCESS));
    client::select(&sdk).await.unwrap();
    assert!(sdk
        .head_object()
        .bucket("another-bucket")
        .key(client::KEY)
        .version_id(VERSION)
        .send()
        .await
        .is_err());
    let requests = fixture.finish().await;
    assert_eq!(requests.len(), 1);
    assert!(requests[0].contains("host: fixture-bucket.localhost:"));
    assert!(requests[0].starts_with("HEAD /models/"));
    assert!(valid_signature(&requests[0], SECRET));
}

#[tokio::test]
async fn streamed_body_is_lazy_and_cancellation_closes_the_owned_connection() {
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
        time::{timeout, Duration},
    };
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let source = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = Vec::new();
        while !request.ends_with(b"\r\n\r\n") {
            assert!(request.len() < 16 * 1024);
            request.push(socket.read_u8().await.unwrap());
        }
        socket.write_all(b"HTTP/1.1 206 Partial Content\r\nContent-Length: 131072\r\nContent-Range: bytes 0-131071/131072\r\nx-amz-version-id: v+1/=\r\nETag: \"selected\"\r\nConnection: close\r\n\r\n").await.unwrap();
        socket.write_all(&[b'x'; 65536]).await.unwrap();
        let mut byte = [0];
        assert_eq!(
            timeout(Duration::from_secs(5), socket.read(&mut byte))
                .await
                .unwrap()
                .unwrap(),
            0
        );
    });
    let sdk = client::build(
        &endpoint,
        Addressing::Path,
        Some(None),
        None,
        None,
        131072,
        true,
    )
    .unwrap();
    let output = timeout(
        Duration::from_secs(2),
        sdk.get_object()
            .bucket(BUCKET)
            .key(client::KEY)
            .version_id(VERSION)
            .range("bytes=0-131071")
            .send(),
    )
    .await
    .unwrap()
    .unwrap();
    let mut body = output.body;
    let mut read = 0;
    while read < 65536 {
        read += body.try_next().await.unwrap().unwrap().len();
    }
    assert_eq!(read, 65536);
    assert!(timeout(Duration::from_millis(50), body.try_next())
        .await
        .is_err());
    drop(body);
    drop(sdk);
    timeout(Duration::from_secs(6), source)
        .await
        .expect("source custody did not drain after body cancellation")
        .unwrap();
}

#[tokio::test]
async fn redirects_access_denial_and_server_errors_are_not_followed_or_retried() {
    let receiving = Fixture::serve(vec![], false).await;
    for status in [301, 302, 307, 308, 403, 500, 503] {
        let body = format!(
            "<Error><Code>AccessDenied</Code><Message>{ACCESS}{SECRET}{TOKEN}</Message></Error>"
        );
        let response=format!("HTTP/1.1 {status} Fixture\r\nContent-Length: {}\r\nContent-Type: application/xml\r\nLocation: {}\r\nx-amz-bucket-region: another-region\r\nConnection: close\r\n\r\n{body}",body.len(),receiving.endpoint);
        let fixture = Fixture::serve(vec![response], false).await;
        let sdk = reader(&fixture, Addressing::Path, Some(Some(TOKEN)), 4096);
        let error = client::select(&sdk).await.unwrap_err();
        assert!(!format!("{error:?}").contains(ACCESS));
        assert_eq!(fixture.finish().await.len(), 1);
    }
    assert!(receiving.finish().await.is_empty());
}

#[tokio::test]
async fn listing_parser_retains_completion_and_continuation_independently() {
    for (flag, token, valid) in [
        ("<IsTruncated>false</IsTruncated>", "", true),
        (
            "<IsTruncated>true</IsTruncated>",
            "<NextContinuationToken>opaque+/=</NextContinuationToken>",
            true,
        ),
        ("<IsTruncated>true</IsTruncated>", "", false),
        ("", "", false),
        (
            "<IsTruncated>false</IsTruncated>",
            "<NextContinuationToken>opaque</NextContinuationToken>",
            false,
        ),
        (
            "<IsTruncated>true</IsTruncated>",
            "<NextContinuationToken></NextContinuationToken>",
            false,
        ),
    ] {
        let body=format!("<ListBucketResult>{flag}{token}<KeyCount>1</KeyCount><Contents><Key>models/a.bin</Key><ETag>\"opaque-multipart-2\"</ETag><Size>8</Size></Contents></ListBucketResult>");
        let fixture = Fixture::serve(vec![fixture::xml(&body)], false).await;
        let sdk = reader(&fixture, Addressing::Path, Some(Some(TOKEN)), 4096);
        let output = sdk
            .list_objects_v2()
            .bucket(BUCKET)
            .prefix("models")
            .max_keys(2)
            .continuation_token("previous+/=")
            .send()
            .await;
        assert_eq!(output.is_ok(), valid);
        if let Ok(output) = output {
            assert!(client::completion(&output).is_ok());
            assert_eq!(output.contents()[0].key(), Some("models/a.bin"));
        }
        let requests = fixture.finish().await;
        assert_eq!(requests.len(), 1);
        assert!(valid_signature(&requests[0], SECRET));
        assert!(requests[0].contains("prefix=models&"));
        assert!(!requests[0].contains("prefix=models%2F"));
        assert!(requests[0].contains("continuation-token=previous%2B%2F%3D"));
        assert!(requests[0].contains("max-keys=2"));
    }
}

#[tokio::test]
async fn invalid_scalar_and_oversized_list_bodies_are_refused() {
    for (body, limit) in [
        (
            "<ListBucketResult><IsTruncated>bogus</IsTruncated></ListBucketResult>",
            4096,
        ),
        (
            "<ListBucketResult><IsTruncated>false</IsTruncated></ListBucketResult>",
            16,
        ),
    ] {
        let fixture = Fixture::serve(vec![fixture::xml(body)], false).await;
        let sdk = reader(&fixture, Addressing::Path, None, limit);
        assert!(sdk
            .list_objects_v2()
            .bucket(BUCKET)
            .prefix("models")
            .send()
            .await
            .is_err());
        assert_eq!(fixture.finish().await.len(), 1);
    }
}

#[tokio::test]
async fn guarded_listing_refuses_malformed_or_ambiguous_completion() {
    for body in [
        "<ListBucketResult><IsTruncated>false</IsTruncated>",
        "<ListBucketResult><IsTruncated>true</IsTruncated><IsTruncated>false</IsTruncated></ListBucketResult>",
        "<WrongRoot><IsTruncated>false</IsTruncated></WrongRoot>",
        "<ListBucketResult><IsTruncated>true</IsTruncated><NextContinuationToken>a</NextContinuationToken><NextContinuationToken>b</NextContinuationToken></ListBucketResult>",
        "<ListBucketResult><IsTruncated>false</IsTruncated><Prefix>models</Prefix><Prefix>other</Prefix></ListBucketResult>",
        "<ListBucketResult><IsTruncated>false</IsTruncated><Contents><Key>models/a</Key><Key>other</Key></Contents></ListBucketResult>",
        "<ListBucketResult><IsTruncated>false</IsTruncated><Contents><Key>models/a</Key><Size>1</Size><Size>2</Size></Contents></ListBucketResult>",
        "<ListBucketResult><IsTruncated>false</IsTruncated><Contents><Key>models/a</Key><ETag>a</ETag><ETag>b</ETag></Contents></ListBucketResult>",
        "<ListBucketResult xmlns='urn:wrong'><IsTruncated>false</IsTruncated></ListBucketResult>",
        "<!DOCTYPE ListBucketResult [<!ENTITY t 'false'>]><ListBucketResult><IsTruncated>&t;</IsTruncated></ListBucketResult>",
    ] {
        let fixture = Fixture::serve(vec![fixture::xml(body)], false).await;
        let sdk = reader(&fixture, Addressing::Path, None, 4096);
        let output = sdk.list_objects_v2().bucket(BUCKET).prefix("models").send().await;
        assert!(output.is_err(), "invalid listing must fail before send returns");
        assert_eq!(fixture.finish().await.len(), 1);
    }
}

#[tokio::test]
async fn listing_guard_preserves_namespace_and_enforces_xml_node_budget() {
    let valid = "<ListBucketResult xmlns='http://s3.amazonaws.com/doc/2006-03-01/'><IsTruncated>true</IsTruncated><NextContinuationToken>next&amp;opaque</NextContinuationToken></ListBucketResult>".to_owned();
    let overflow = format!(
        "<ListBucketResult><IsTruncated>false</IsTruncated>{}</ListBucketResult>",
        "<Ignored/>".repeat(4096)
    );
    for (body, accepted) in [(valid, true), (overflow, false)] {
        let fixture = Fixture::serve(vec![fixture::xml(&body)], false).await;
        let sdk = reader(&fixture, Addressing::Path, None, 64 * 1024);
        let result = client::page(&sdk).await;
        assert_eq!(result.is_ok(), accepted);
        if let Ok(output) = result {
            assert_eq!(client::completion(&output).unwrap(), Some("next&opaque"));
        }
        assert_eq!(fixture.finish().await.len(), 1);
    }
}

#[derive(Clone)]
struct MemoryLog(Arc<Mutex<Vec<u8>>>);

#[tokio::test]
async fn listing_guard_refuses_sdk_xml_value_disagreement() {
    for field in [
        "<Contents><Key>models/a<![CDATA[b]]></Key><ETag>\"opaque\"</ETag><Size>1</Size></Contents>",
        "<Contents><Key><![CDATA[models/a]]></Key><ETag>\"opaque\"</ETag><Size>1</Size></Contents>",
        "<Contents><Key>models/a</Key><ETag>opaque<![CDATA[changed]]></ETag><Size>1</Size></Contents>",
        "<Contents><Key>models/a</Key><Size>1<![CDATA[2]]></Size></Contents>",
        "<Prefix>models/<![CDATA[other]]></Prefix>",
        "<KeyCount>1<![CDATA[2]]></KeyCount>",
        "<CommonPrefixes><Prefix>models/<![CDATA[other]]></Prefix></CommonPrefixes>",
    ] {
        let body = format!("<ListBucketResult><IsTruncated>false</IsTruncated>{field}</ListBucketResult>");
        let fixture = Fixture::serve(vec![fixture::xml(&body)], false).await;
        let sdk = reader(&fixture, Addressing::Path, None, 4096);
        let result = client::page(&sdk).await;
        assert!(result.is_err(), "different SDK/XML selection values must be refused");
        assert_eq!(fixture.finish().await.len(), 1);
    }
    let body = "<ListBucketResult><IsTruncated>false</IsTruncated><Name>fixture-bucket</Name><Prefix>models</Prefix><Delimiter>/</Delimiter><ContinuationToken>previous</ContinuationToken><StartAfter>models/0</StartAfter><EncodingType>url</EncodingType><KeyCount>2</KeyCount><MaxKeys>2</MaxKeys><Contents><Key>models/a&amp;1</Key><ETag>\"first\"</ETag><Size>0</Size></Contents><Contents><Key>models/b</Key><ETag>\"second\"</ETag><Size>1</Size></Contents></ListBucketResult>";
    let fixture = Fixture::serve(vec![fixture::xml(body)], false).await;
    let sdk = reader(&fixture, Addressing::Path, None, 4096);
    let output = client::page(&sdk).await.unwrap();
    assert_eq!(
        output
            .contents()
            .iter()
            .map(|item| (item.key(), item.e_tag(), item.size()))
            .collect::<Vec<_>>(),
        vec![
            (Some("models/a&1"), Some("\"first\""), Some(0)),
            (Some("models/b"), Some("\"second\""), Some(1))
        ]
    );
    assert_eq!(fixture.finish().await.len(), 1);
}
impl Write for MemoryLog {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for MemoryLog {
    type Writer = Self;
    fn make_writer(&'a self) -> Self {
        self.clone()
    }
}

#[tokio::test]
async fn probe_sdk_trace_exposes_access_key_id_before_transport_redaction() {
    let memory = MemoryLog(Arc::new(Mutex::new(Vec::new())));
    let subscriber = tracing_subscriber::fmt()
        .without_time()
        .with_ansi(false)
        .with_max_level(tracing::Level::TRACE)
        .with_writer(memory.clone())
        .finish();
    let fixture = Fixture::serve(vec![fixture::head(VERSION, "\"selected\"")], false).await;
    let sdk = reader(&fixture, Addressing::Path, Some(Some(TOKEN)), 4096);
    sdk.head_object()
        .bucket(BUCKET)
        .key(client::KEY)
        .version_id(VERSION)
        .send()
        .with_subscriber(subscriber)
        .await
        .unwrap();
    fixture.finish().await;
    let buffer = memory.0.lock().unwrap();
    let logs = String::from_utf8_lossy(&buffer);
    // Expected incompatibility reproduction. Never write captured credentials or
    // the raw trace to disk/stdout; the decision report records only these facts.
    assert!(
        logs.contains(ACCESS),
        "SDK trace exposure was not reproduced; reassess candidate evidence"
    );
    assert!(!logs.contains(SECRET));
    assert!(!logs.contains(TOKEN));
}

#[test]
fn sdk_future_scope_preserves_safe_statuses_under_global_trace() {
    let result = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "probes::global_trace_scope_child", "--ignored"])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "isolated global-TRACE fixture failed"
    );
    assert!(String::from_utf8_lossy(&result.stdout).contains("1 passed"));
}

#[tokio::test]
#[ignore = "fresh-process global subscriber fixture; parent executes it"]
async fn global_trace_scope_child() {
    let memory = MemoryLog(Arc::new(Mutex::new(Vec::new())));
    let subscriber = tracing_subscriber::fmt()
        .without_time()
        .with_ansi(false)
        .with_max_level(tracing::Level::TRACE)
        .with_writer(memory.clone())
        .finish();
    // Test-only setup in a disposable process; the adapter never changes the
    // embedding application's process-global subscriber.
    tracing::subscriber::set_global_default(subscriber).unwrap();
    let fixture = Fixture::serve(vec![fixture::head(VERSION, "\"selected\"")], true).await;
    let sdk = reader(&fixture, Addressing::Path, Some(Some(TOKEN)), 4096);
    sdk.head_object()
        .bucket(BUCKET)
        .key(client::KEY)
        .version_id(VERSION)
        .send()
        .await
        .unwrap();
    fixture.finish().await;
    assert!(
        String::from_utf8_lossy(&memory.0.lock().unwrap()).contains(ACCESS),
        "global TRACE control must reproduce access-key-ID disclosure"
    );
    memory.0.lock().unwrap().clear();

    let reflected = format!(
        "<Error><Code>AccessDenied</Code><Message>{ACCESS} {SECRET} {TOKEN}</Message></Error>"
    );
    let denied = format!(
        "HTTP/1.1 403 Denied\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{reflected}",
        reflected.len()
    );
    let malformed = format!("<ListBucketResult><IsTruncated>false</IsTruncated><IsTruncated>false</IsTruncated><Prefix>{ACCESS} {SECRET} {TOKEN}</Prefix></ListBucketResult>");
    let fixture = Fixture::serve(vec![
        fixture::head(VERSION, "\"selected\""),
        fixture::range(VERSION, "\"selected\"", 206, Some("bytes 2-5/8"), "cdef"),
        fixture::xml("<ListBucketResult><IsTruncated>true</IsTruncated><NextContinuationToken>next+/=</NextContinuationToken></ListBucketResult>"),
        denied.clone(), denied.clone(), denied, fixture::xml(&malformed),
    ], true).await;
    let sdk = reader(&fixture, Addressing::Path, Some(Some(TOKEN)), 4096);
    tracing::info!(phase = "selecting", "Pumas safe acquisition status");
    let selected = client::select(&sdk).await.unwrap();
    assert_eq!(
        client::range(&sdk, selected.0, &selected.1).await.unwrap(),
        b"cdef"
    );
    assert_eq!(
        client::completion(&client::page(&sdk).await.unwrap()).unwrap(),
        Some("next+/=")
    );
    assert_eq!(client::select(&sdk).await.unwrap_err(), "selection failed");
    assert_eq!(
        client::range(&sdk, 8, "\"selected\"").await.unwrap_err(),
        "range request failed"
    );
    assert_eq!(client::page(&sdk).await.unwrap_err(), "listing failed");
    assert_eq!(client::page(&sdk).await.unwrap_err(), "listing failed");
    tracing::info!(phase = "settled", "Pumas safe acquisition status");
    assert_eq!(fixture.finish().await.len(), 7);
    let buffer = memory.0.lock().unwrap();
    let logs = String::from_utf8_lossy(&buffer);
    assert!(
        logs.contains("selecting") && logs.contains("settled"),
        "safe outer statuses must remain visible"
    );
    assert!(
        !logs.contains(ACCESS),
        "scoped SDK execution disclosed access-key ID"
    );
    assert!(
        !logs.contains(SECRET),
        "scoped SDK execution disclosed secret key"
    );
    assert!(
        !logs.contains(TOKEN),
        "scoped SDK execution disclosed session token"
    );
}

#[test]
fn ambient_credentials_and_proxy_settings_are_ignored_in_fresh_process() {
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "probes::ambient_fixture_child",
            "--ignored",
            "--nocapture",
        ])
        .env("PUMAS_S3_SPIKE_AMBIENT_CHILD", "1")
        .env("AWS_ACCESS_KEY_ID", "ambient-synthetic-access")
        .env("AWS_SECRET_ACCESS_KEY", "ambient-synthetic-secret")
        .env("AWS_SESSION_TOKEN", "ambient-synthetic-token")
        .env("AWS_REGION", "ambient-region")
        .env("AWS_PROFILE", "absent-profile")
        .env(
            "AWS_SHARED_CREDENTIALS_FILE",
            "/absent/pumas-spike-credentials",
        )
        .env("AWS_CONFIG_FILE", "/absent/pumas-spike-config")
        .env("HTTP_PROXY", "http://127.0.0.1:1")
        .env("HTTPS_PROXY", "http://127.0.0.1:1")
        .env("ALL_PROXY", "http://127.0.0.1:1")
        .env("NO_PROXY", "")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "isolated ambient fixture failed: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed; 0 failed"));
}
#[tokio::test]
#[ignore = "owned fresh-process fixture"]
async fn ambient_fixture_child() {
    assert_eq!(std::env::var("PUMAS_S3_SPIKE_AMBIENT_CHILD").unwrap(), "1");
    for auth in [None, Some(Some(TOKEN))] {
        let fixture = Fixture::serve(vec![fixture::head(VERSION, "\"selected\"")], true).await;
        let sdk = reader(&fixture, Addressing::Path, auth, 4096);
        client::select(&sdk).await.unwrap();
        let requests = fixture.finish().await;
        assert_eq!(requests.len(), 1);
        assert!(!requests[0].contains("ambient-synthetic"));
        assert_eq!(requests[0].contains("authorization:"), auth.is_some());
    }
}
