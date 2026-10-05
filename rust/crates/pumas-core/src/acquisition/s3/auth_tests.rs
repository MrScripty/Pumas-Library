//! Synthetic credentials only. Plaintext transport is private to cfg(test).
use super::*;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

#[path = "../../../tests/s3_reader/fixture.rs"]
mod fixture;
use fixture::{head, range_response, Fixture};

const ACCESS: &str = "PUMAS-SYNTHETIC-ACCESS";
const SECRET: &str = "pumas-synthetic-secret/+=";
const TOKEN: &str = "pumas-synthetic-session/+=";
const KEY: &str = "models/a b%?.bin";
const VERSION: &str = "v+1/=";

fn config(endpoint: String, addressing: S3Addressing) -> S3ReaderConfig {
    S3ReaderConfig {
        endpoint,
        region: "fixture-region".into(),
        bucket: "fixture-bucket".into(),
        addressing,
        allow_http: true,
        operation_timeout: Duration::from_secs(5),
    }
}
fn credentials(token: Option<&str>) -> S3Credentials {
    S3Credentials::new(ACCESS.into(), SECRET.into(), token.map(str::to_owned)).unwrap()
}
fn reader(endpoint: String, addressing: S3Addressing, token: Option<&str>) -> S3Reader {
    S3Reader::build(
        config(endpoint, addressing),
        Authentication::LoopbackFixture(credentials(token)),
    )
    .unwrap()
}
fn digest() -> Sha256Evidence {
    Sha256Evidence::new("fixture.sha256", hex::encode(Sha256::digest(b"abcdefgh"))).unwrap()
}

// Test-only RFC 2104 HMAC oracle using the existing maintained SHA-256 primitive.
// Production signing is exclusively object_store. The known vector below anchors
// this small oracle independently of the SDK before checking captured requests.
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
    let inner: Vec<u8> = block
        .iter()
        .map(|b| b ^ 0x36)
        .chain(data.iter().copied())
        .collect();
    let outer: Vec<u8> = block
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
    let authorization = &headers["authorization"];
    let fields: Vec<_> = authorization
        .strip_prefix("AWS4-HMAC-SHA256 ")
        .unwrap()
        .split(", ")
        .collect();
    let credential = fields[0].strip_prefix("Credential=").unwrap();
    let (access, scope) = credential.split_once('/').unwrap();
    assert_eq!(access, ACCESS);
    let signed = fields[1].strip_prefix("SignedHeaders=").unwrap();
    let expected = fields[2].strip_prefix("Signature=").unwrap();
    let canonical_headers: String = signed
        .split(';')
        .map(|name| format!("{name}:{}\n", headers[name]))
        .collect();
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
    hex::encode(hmac(&signing_key, to_sign.as_bytes())) == expected
}

#[test]
fn signature_oracle_matches_rfc4231_case_one() {
    assert_eq!(
        hex::encode(hmac(&[0x0b; 20], b"Hi There")),
        "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7"
    );
}

#[tokio::test]
async fn signs_version_bound_head_and_conditional_range_with_optional_token() {
    for addressing in [S3Addressing::Path, S3Addressing::VirtualHosted] {
        for token in [None, Some(TOKEN)] {
            let fixture = Fixture::serve(vec![
                head(Some(VERSION), Some("\"selected\"")),
                range_response(VERSION, "\"selected\"", 8, "2-4", "cde"),
            ])
            .await;
            let reader = reader(fixture.endpoint.clone(), addressing, token);
            let selection = reader
                .select(KEY, VERSION, "weights.bin", digest())
                .await
                .unwrap();
            // Selections keep the scoped capability alive after construction owner drops.
            drop(reader);
            let mut bytes = Vec::new();
            assert_eq!(selection.read_range(2..5, &mut bytes).await.unwrap(), 3);
            assert_eq!(bytes, b"cde");
            let requests = fixture.finish().await;
            assert_eq!(requests.len(), 2);
            for request in &requests {
                assert!(valid_signature(request, SECRET));
                assert!(!valid_signature(request, "wrong-synthetic-secret"));
                assert!(!valid_signature(
                    &request.replace("versionId=v%2B1%2F%3D", "versionId=other"),
                    SECRET
                ));
                assert!(!valid_signature(
                    &request.replace("models/a%20b%25%3F.bin", "models/other.bin"),
                    SECRET
                ));
                let token_line = format!("x-amz-security-token: {TOKEN}\r\n");
                assert_eq!(request.contains(&token_line), token.is_some());
                assert!(!request.contains(SECRET));
            }
            assert!(!valid_signature(
                &requests[1].replace("bytes=2-4", "bytes=1-4"),
                SECRET
            ));
            assert!(!valid_signature(
                &requests[1].replace("\"selected\"", "\"changed\""),
                SECRET
            ));
            assert!(requests[1].contains("SignedHeaders="));
        }
    }
}

#[test]
fn credential_validation_and_debug_never_disclose_supplied_values() {
    let debug = format!("{:?}", credentials(Some(TOKEN)));
    for value in [ACCESS, SECRET, TOKEN] {
        assert!(!debug.contains(value));
    }
    for (key, secret, token) in [
        ("", SECRET, None),
        (ACCESS, "", None),
        ("key/invalid", SECRET, None),
        ("key\r\n", SECRET, None),
        (ACCESS, SECRET, Some("")),
        (ACCESS, SECRET, Some("token\r\nInjected: yes")),
        (ACCESS, SECRET, Some("nonascii-ø")),
    ] {
        let error =
            S3Credentials::new(key.into(), secret.into(), token.map(str::to_owned)).unwrap_err();
        assert!(matches!(
            error,
            S3ReaderError::Configuration("invalid explicit credentials")
        ));
        assert_eq!(
            error.to_string(),
            "invalid S3 source configuration: invalid explicit credentials"
        );
    }
}

#[test]
fn plaintext_fixture_is_literal_loopback_only_and_signing_region_is_safe() {
    for endpoint in [
        "http://localhost:1",
        "http://192.0.2.1:1",
        "https://127.0.0.1:1",
    ] {
        let error = S3Reader::build(
            config(endpoint.into(), S3Addressing::Path),
            Authentication::LoopbackFixture(credentials(None)),
        )
        .err()
        .unwrap();
        assert!(matches!(
            error,
            S3ReaderError::Configuration("fixture requires literal loopback HTTP")
        ));
    }
    for region in ["bad region", "bad/region", "région"] {
        let mut config = config("https://example.invalid".into(), S3Addressing::Path);
        config.region = region.into();
        assert!(matches!(
            S3Reader::new_authenticated(config, credentials(None)),
            Err(S3ReaderError::Configuration("invalid signing region"))
        ));
    }
}

#[tokio::test]
async fn authenticated_failures_are_redacted_and_never_redirected_or_retried() {
    let target = Fixture::serve(vec![]).await;
    for status in [
        "302 Found",
        "307 Temporary Redirect",
        "400 Bad Request",
        "403 Forbidden",
        "503 Service Unavailable",
    ] {
        let body = format!("echo {ACCESS} {SECRET} {TOKEN}");
        let response = format!("HTTP/1.1 {status}\r\nLocation: {}/leak\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", target.endpoint, body.len());
        let fixture = Fixture::serve(vec![response]).await;
        let reader = reader(fixture.endpoint.clone(), S3Addressing::Path, Some(TOKEN));
        let error = reader
            .select(KEY, VERSION, "weights.bin", digest())
            .await
            .err()
            .unwrap();
        assert!(matches!(error, S3ReaderError::Protocol(_)));
        let mut public = format!("{error} {error:?}");
        public.push_str(&format!(" {:?}", acquisition_error(error)));
        for value in [ACCESS, SECRET, TOKEN] {
            assert!(!public.contains(value));
        }
        let requests = fixture.finish().await;
        assert_eq!(requests.len(), 1);
        assert!(valid_signature(&requests[0], SECRET));
    }
    assert!(target.finish().await.is_empty());
}

#[tokio::test]
async fn authentication_does_not_change_manifest_or_receipt_identity() {
    check_identity().await;
}

async fn check_identity() {
    let fixture = Fixture::serve(vec![head(Some(VERSION), Some("\"selected\"")); 3]).await;
    let config = || config(fixture.endpoint.clone(), S3Addressing::Path);
    let anonymous = S3Reader::new(config())
        .unwrap()
        .select(KEY, VERSION, "weights.bin", digest())
        .await
        .unwrap();
    let signed = reader(fixture.endpoint.clone(), S3Addressing::Path, None)
        .select(KEY, VERSION, "weights.bin", digest())
        .await
        .unwrap();
    let temporary = reader(fixture.endpoint.clone(), S3Addressing::Path, Some(TOKEN))
        .select(KEY, VERSION, "weights.bin", digest())
        .await
        .unwrap();
    assert_eq!(anonymous.manifest(), signed.manifest());
    assert_eq!(anonymous.manifest(), temporary.manifest());
    assert_eq!(
        anonymous.acquisition_identity(),
        signed.acquisition_identity()
    );
    let json = serde_json::to_string(signed.manifest()).unwrap();
    for value in [ACCESS, SECRET, TOKEN] {
        assert!(!json.contains(value));
    }
    let requests = fixture.finish().await;
    assert!(!requests[0].contains("authorization:"));
    assert!(!requests[0].contains("x-amz-security-token:"));
    assert!(valid_signature(&requests[1], SECRET));
    assert!(valid_signature(&requests[2], SECRET));
}

#[tokio::test]
async fn ambient_credentials_and_proxies_are_ignored_in_isolated_process() {
    const MARKER: &str = "PUMAS_S3_AUTH_CHILD";
    if std::env::var_os(MARKER).is_none() {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "acquisition::s3::auth_tests::ambient_credentials_and_proxies_are_ignored_in_isolated_process", "--nocapture"])
            .env(MARKER, "1")
            .env("AWS_ACCESS_KEY_ID", "ambient-synthetic-key")
            .env("AWS_SECRET_ACCESS_KEY", "ambient-synthetic-secret")
            .env("AWS_SESSION_TOKEN", "ambient-synthetic-token")
            .env("AWS_ENDPOINT_URL", "http://127.0.0.1:1")
            .env("AWS_CONTAINER_CREDENTIALS_FULL_URI", "http://127.0.0.1:1")
            .env("AWS_WEB_IDENTITY_TOKEN_FILE", "/nonexistent-pumas-s3-fixture")
            .env("AWS_ROLE_ARN", "synthetic-role")
            .env("AWS_REGION", "ambient-region")
            .env("HTTP_PROXY", "http://127.0.0.1:1")
            .env("HTTPS_PROXY", "http://127.0.0.1:1")
            .env("ALL_PROXY", "http://127.0.0.1:1")
            .env("NO_PROXY", "")
            .status().unwrap();
        assert!(status.success());
        return;
    }
    check_identity().await;
}

struct Host;
#[async_trait::async_trait]
impl super::super::HttpAttemptHost for Host {
    async fn pause_requested(&self) {
        std::future::pending::<()>().await;
    }
    fn pause_requested_now(&self) -> bool {
        false
    }
    fn cancel_requested(&self) -> bool {
        false
    }
    async fn record_progress(&mut self, _: u64) -> crate::Result<()> {
        Ok(())
    }
}
#[async_trait::async_trait]
impl super::super::AcquisitionHost for Host {
    async fn retry(&mut self, _: u32, _: Option<Duration>, _: Option<&str>) -> crate::Result<()> {
        Ok(())
    }
}

#[tokio::test]
async fn authenticated_shared_acquisition_verifies_and_persists_only_nonsecret_receipts() {
    use super::super::{
        AcquisitionDemand, AcquisitionPhase, AcquisitionRetryPolicy, AcquisitionS3Request,
        AcquisitionService, AcquisitionStore, AcquisitionWorkspace,
    };
    let state = tempfile::TempDir::new().unwrap();
    let stage = tempfile::TempDir::new().unwrap();
    std::fs::create_dir(stage.path().join("stage")).unwrap();
    let store = Arc::new(AcquisitionStore::new(state.path()));
    let service = Arc::new(AcquisitionService::new(store.clone()));
    let consumer = service.open_consumer("fixture.s3.auth").unwrap();
    let fixture = Fixture::serve(vec![
        head(Some(VERSION), Some("\"selected\"")),
        range_response(VERSION, "\"selected\"", 8, "0-7", "abcdefgh"),
    ])
    .await;
    let reader = reader(fixture.endpoint.clone(), S3Addressing::Path, Some(TOKEN));
    let selected = reader
        .select(KEY, VERSION, "weights.bin", digest())
        .await
        .unwrap();
    let manifest = selected.manifest().clone();
    let workspace = AcquisitionWorkspace::from_reserved_directory(
        stage.path(),
        std::path::Path::new("stage"),
        Arc::new(()),
        || Ok(()),
    )
    .unwrap();
    let receipt = consumer
        .acquire_s3(
            AcquisitionS3Request {
                demand: AcquisitionDemand {
                    consumer: "fixture.s3.auth".into(),
                    operation: "read".into(),
                },
                selection: selected,
                workspace,
                retry: AcquisitionRetryPolicy {
                    attempts: Some(1),
                    elapsed: Duration::from_secs(5),
                    backoff: crate::network::RetryConfig::new(),
                },
            },
            Box::new(Host),
            |acquired| async move {
                assert_eq!(acquired.record().files[0].bytes, 8);
                Ok((
                    acquired,
                    serde_json::json!({"result": "synthetic verified bytes"}),
                ))
            },
            |acquired, receipt| async move {
                assert_eq!(receipt.verified_files, acquired.record().files);
                Ok(receipt)
            },
        )
        .await
        .unwrap();
    let record = store.acquisitions().unwrap().into_values().next().unwrap();
    assert!(matches!(record.phase, AcquisitionPhase::Adopted { .. }));
    assert_eq!(receipt.manifest, manifest);
    assert_eq!(receipt.manifest, record.manifest);
    assert_eq!(
        consumer.completion_receipt(&record).unwrap(),
        Some(receipt.clone())
    );
    assert_eq!(
        std::fs::read(stage.path().join("stage/weights.bin")).unwrap(),
        b"abcdefgh"
    );
    let persisted = std::fs::read_to_string(state.path().join("downloads.json")).unwrap();
    let diagnostics = format!("{receipt:?} {record:?} {persisted}");
    for value in [ACCESS, SECRET, TOKEN] {
        assert!(!diagnostics.contains(value));
    }
    consumer.shutdown().await.unwrap();
    service.shutdown().await.unwrap();
    drop(consumer);
    drop(service);
    drop(store);
    drop(reader);
    let reopened = AcquisitionStore::new(state.path());
    assert_eq!(
        reopened.acquisitions().unwrap().get(&record.id),
        Some(&record)
    );
    let requests = fixture.finish().await;
    assert_eq!(requests.len(), 2);
    assert!(requests
        .iter()
        .all(|request| valid_signature(request, SECRET)));
}

#[tokio::test]
async fn malformed_success_metadata_and_range_body_errors_remain_redacted() {
    // GET's upstream parse error contains the untrusted header value.
    let bad = range_response(VERSION, "\"selected\"", 8, TOKEN, "abcdefgh");
    let fixture = Fixture::serve(vec![head(Some(VERSION), Some("\"selected\"")), bad]).await;
    let selected = reader(fixture.endpoint.clone(), S3Addressing::Path, Some(TOKEN))
        .select(KEY, VERSION, "weights.bin", digest())
        .await
        .unwrap();
    let mut output = Vec::new();
    let error = selected.read_range(0..8, &mut output).await.unwrap_err();
    assert!(matches!(error, S3ReaderError::Protocol(_)));
    assert!(!format!("{error} {error:?}").contains(TOKEN));
    assert!(output.is_empty());
    assert_eq!(fixture.finish().await.len(), 2);
}
