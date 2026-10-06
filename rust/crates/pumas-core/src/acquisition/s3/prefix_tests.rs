//! Synthetic source fixtures; no ambient or real account credentials.
use super::{
    auth_tests::{self, ACCESS, SECRET, TOKEN},
    *,
};
use sha2::{Digest, Sha256};

use super::fixture::{self, head, range_response, Fixture};

fn limits() -> S3PrefixLimits {
    S3PrefixLimits {
        page_size: 2,
        max_pages: 3,
        max_objects: 8,
        max_page_bytes: 4096,
        max_total_bytes: 12288,
    }
}
fn xml(value: &str) -> String {
    format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/xml\r\nConnection: close\r\n\r\n{value}", value.len())
}
fn escape(value: &str) -> String {
    value.replace('&', "&amp;").replace('<', "&lt;")
}
fn page(
    prefix: &str,
    keys: &[(&str, i64)],
    truncated: bool,
    token: Option<&str>,
    next: Option<&str>,
    max_keys: u16,
) -> String {
    let contents = keys
        .iter()
        .map(|(key, size)| {
            format!(
                "<Contents><Key>{}</Key><ETag>\"selected\"</ETag><Size>{size}</Size></Contents>",
                escape(key)
            )
        })
        .collect::<String>();
    format!("<ListBucketResult><Name>fixture-bucket</Name><Prefix>{}</Prefix><MaxKeys>{max_keys}</MaxKeys><KeyCount>{}</KeyCount><IsTruncated>{truncated}</IsTruncated>{}{}{contents}</ListBucketResult>",
        escape(prefix), keys.len(), token.map(|token| format!("<ContinuationToken>{}</ContinuationToken>",escape(token))).unwrap_or_default(),
        next.map(|token|format!("<NextContinuationToken>{}</NextContinuationToken>",escape(token))).unwrap_or_default())
}
fn anonymous(endpoint: String) -> S3Reader {
    S3Reader::new(auth_tests::config(endpoint, S3Addressing::Path)).unwrap()
}
fn digest() -> Sha256Evidence {
    Sha256Evidence::new("fixture.sha256", hex::encode(Sha256::digest(b"abcdefgh"))).unwrap()
}

#[tokio::test]
async fn invalid_prefix_and_capacity_fail_before_io() {
    let fixture = Fixture::serve(vec![]).await;
    let reader = anonymous(fixture.endpoint.clone());
    for prefix in ["x".repeat(1025), "models\n".into()] {
        assert!(matches!(
            reader.enumerate_prefix(&prefix, limits()).await,
            Err(S3PrefixError::Reader(S3ReaderError::Configuration(_)))
        ));
    }
    for invalid in [
        S3PrefixLimits {
            page_size: 0,
            ..limits()
        },
        S3PrefixLimits {
            page_size: 1001,
            ..limits()
        },
        S3PrefixLimits {
            max_pages: 0,
            ..limits()
        },
        S3PrefixLimits {
            max_objects: 0,
            ..limits()
        },
        S3PrefixLimits {
            max_page_bytes: 0,
            ..limits()
        },
        S3PrefixLimits {
            max_page_bytes: 1048577,
            ..limits()
        },
        S3PrefixLimits {
            max_total_bytes: 0,
            ..limits()
        },
    ] {
        assert!(matches!(
            reader.enumerate_prefix("models", invalid).await,
            Err(S3PrefixError::Reader(S3ReaderError::Configuration(_)))
        ));
    }
    let mut config = auth_tests::config(fixture.endpoint.clone(), S3Addressing::Path);
    config.bucket = "fixture-bucket--zone--x-s3".into();
    assert!(matches!(
        S3Reader::new(config),
        Err(S3ReaderError::Configuration(_))
    ));
    assert!(fixture.finish().await.is_empty());
}

#[tokio::test]
async fn complete_pages_pin_versions_and_preserve_anonymous_signed_manifest_identity() {
    for addressing in [S3Addressing::Path, S3Addressing::VirtualHosted] {
        let first = page(
            "models",
            &[("models/a b%?.bin", 8)],
            true,
            None,
            Some("opaque+&/="),
            1,
        );
        let second = page(
            "models",
            &[("models/z.bin", 8)],
            false,
            Some("opaque+&/="),
            None,
            1,
        );
        let responses = vec![
            xml(&first),
            xml(&second),
            head(Some("v+1/="), Some("\"selected\"")),
            head(Some("v2"), Some("\"selected\"")),
            head(Some("v+1/="), Some("\"selected\"")),
            head(Some("v2"), Some("\"selected\"")),
            range_response("v+1/=", "\"selected\"", 8, "0-7", "abcdefgh"),
            range_response("v2", "\"selected\"", 8, "0-7", "abcdefgh"),
        ];
        let fixture = Fixture::serve(std::iter::repeat_n(responses, 3).flatten().collect()).await;
        let mut manifests = vec![];
        for auth in [None, Some(None), Some(Some(TOKEN))] {
            let reader = match auth {
                None => {
                    S3Reader::new(auth_tests::config(fixture.endpoint.clone(), addressing)).unwrap()
                }
                Some(token) => auth_tests::reader(fixture.endpoint.clone(), addressing, token),
            };
            let listing = reader
                .enumerate_prefix(
                    "models",
                    S3PrefixLimits {
                        page_size: 1,
                        ..limits()
                    },
                )
                .await
                .unwrap();
            assert_eq!(listing.pages(), 2);
            assert_eq!(listing.xml_bytes(), first.len() + second.len());
            assert_eq!(
                listing
                    .objects()
                    .iter()
                    .map(|o| (o.key(), o.version(), o.etag(), o.size()))
                    .collect::<Vec<_>>(),
                vec![
                    ("models/a b%?.bin", "v+1/=", "\"selected\"", 8),
                    ("models/z.bin", "v2", "\"selected\"", 8)
                ]
            );
            assert!(!format!("{listing:?} {:?}", listing.objects()).contains("models/"));
            let entries = listing
                .objects()
                .iter()
                .enumerate()
                .map(|(i, o)| o.manifest_entry(format!("file{i}.bin"), digest()))
                .collect();
            let selected = reader.select_manifest(entries).await.unwrap();
            manifests.push(selected.manifest().clone());
            for object in &selected.objects {
                let mut bytes = vec![];
                object.read_range(0..8, &mut bytes).await.unwrap();
                assert_eq!(bytes, b"abcdefgh");
            }
        }
        assert!(manifests.windows(2).all(|pair| pair[0] == pair[1]));
        let requests = fixture.finish().await;
        assert_eq!(requests.len(), 24);
        for (i, request) in requests.iter().enumerate() {
            let signed = i >= 8;
            assert_eq!(
                request.to_ascii_lowercase().contains("authorization:"),
                signed
            );
            assert_eq!(
                request.contains(&format!("x-amz-security-token: {TOKEN}")),
                i >= 16
            );
            if signed {
                assert!(auth_tests::valid_signature(request, SECRET));
                assert!(!auth_tests::valid_signature(
                    request,
                    "wrong synthetic secret"
                ));
            }
            if i % 8 < 2 {
                assert!(request.contains("list-type=2"));
                let target = request
                    .lines()
                    .next()
                    .unwrap()
                    .split_whitespace()
                    .nth(1)
                    .unwrap();
                let pairs =
                    url::form_urlencoded::parse(target.split_once('?').unwrap().1.as_bytes())
                        .collect::<Vec<_>>();
                assert!(pairs
                    .iter()
                    .any(|(key, value)| key == "prefix" && value == "models"));
                assert!(!request.contains("range:"));
                if i % 8 == 1 {
                    assert!(pairs
                        .iter()
                        .any(|(key, value)| key == "continuation-token" && value == "opaque+&/="));
                }
            } else if i % 8 < 4 {
                assert!(request.contains("if-match: \"selected\""));
                assert!(!request.contains("versionId="));
            } else {
                assert!(request.contains("versionId="));
            }
            let expected = match addressing {
                S3Addressing::Path => "/fixture-bucket",
                S3Addressing::VirtualHosted => "/",
            };
            assert!(request.lines().next().unwrap().contains(expected));
        }
    }
}

#[tokio::test]
async fn malformed_or_contradictory_completion_never_pins_objects() {
    let valid = page("models", &[("models/a.bin", 8)], false, None, None, 2);
    for fragment in ["", "<IsTruncated>true</IsTruncated>", "<IsTruncated>0</IsTruncated>",
        "<IsTruncated>false</IsTruncated><NextContinuationToken>unused</NextContinuationToken>",
        "<IsTruncated>true</IsTruncated><NextContinuationToken/>",
        "<IsTruncated>false</IsTruncated><IsTruncated>true</IsTruncated>",
        "<IsTruncated>false</IsTruncated><IsTruncated>false</IsTruncated>",
        "<IsTruncated>true</IsTruncated><NextContinuationToken>one</NextContinuationToken><NextContinuationToken>two</NextContinuationToken>",
        "<IsTruncated>false<![CDATA[true]]></IsTruncated>",
    ] {
        let body=valid.replace("<IsTruncated>false</IsTruncated>",fragment);
        let fixture=Fixture::serve(vec![xml(&body)]).await;
        assert!(anonymous(fixture.endpoint.clone()).enumerate_prefix("models",limits()).await.is_err());
        assert_eq!(fixture.finish().await.len(),1);
    }
}

#[tokio::test]
async fn reviewed_xml_sdk_agreement_and_structural_guards_refuse_ambiguous_selection() {
    let valid = page("models", &[("models/a.bin", 8)], false, None, None, 2);
    let changes = [
        (
            "<Key>models/a.bin</Key>",
            "<Key>models/a<![CDATA[.bin]]></Key>",
        ),
        (
            "<Key>models/a.bin</Key>",
            "<Key><![CDATA[models/a.bin]]></Key>",
        ),
        (
            "<Key>models/a.bin</Key>",
            "<Key>models/a.bin</Key><Key>models/b.bin</Key>",
        ),
        (
            "<Key>models/a.bin</Key>",
            "<Key xmlns='unexpected'>models/a.bin</Key>",
        ),
        (
            "<Key>models/a.bin</Key>",
            "<Key>models/a.bin<Nested/></Key>",
        ),
        (
            "<ETag>\"selected\"</ETag>",
            "<ETag>\"selected<![CDATA[changed]]>\"</ETag>",
        ),
        ("<Size>8</Size>", "<Size>8<![CDATA[0]]></Size>"),
        ("<Size>8</Size>", "<Size>8</Size><Size>9</Size>"),
        (
            "<Prefix>models</Prefix>",
            "<Prefix>models<![CDATA[/other]]></Prefix>",
        ),
        (
            "<KeyCount>1</KeyCount>",
            "<KeyCount>1<![CDATA[2]]></KeyCount>",
        ),
        (
            "<MaxKeys>2</MaxKeys>",
            "<MaxKeys>2</MaxKeys><MaxKeys>3</MaxKeys>",
        ),
        (
            "<ListBucketResult>",
            "<ListBucketResult xmlns='unexpected'>",
        ),
        (
            "<ListBucketResult>",
            "<!DOCTYPE ListBucketResult [<!ENTITY x 'models/a.bin'>]><ListBucketResult>",
        ),
        ("<Contents>", "<Contents xmlns='unexpected'>"),
    ];
    for (from, to) in changes {
        let fixture = Fixture::serve(vec![xml(&valid.replace(from, to))]).await;
        assert!(anonymous(fixture.endpoint.clone())
            .enumerate_prefix("models", limits())
            .await
            .is_err());
        assert_eq!(fixture.finish().await.len(), 1);
    }
    let overflow = valid.replace(
        "</ListBucketResult>",
        &format!("{}</ListBucketResult>", "<Ignored/>".repeat(4096)),
    );
    let fixture = Fixture::serve(vec![xml(&overflow)]).await;
    assert!(anonymous(fixture.endpoint.clone())
        .enumerate_prefix(
            "models",
            S3PrefixLimits {
                max_page_bytes: 65536,
                max_total_bytes: 65536,
                ..limits()
            }
        )
        .await
        .is_err());
    assert_eq!(fixture.finish().await.len(), 1);
}

#[tokio::test]
async fn selection_echo_and_non_recursive_shapes_must_match_request() {
    let valid = page("models", &[("models/a.bin", 8)], false, None, None, 2);
    for (from, to) in [
        ("<Name>fixture-bucket</Name>", "<Name>other-bucket</Name>"),
        ("<Prefix>models</Prefix>", "<Prefix>other</Prefix>"),
        ("<MaxKeys>2</MaxKeys>", "<MaxKeys>3</MaxKeys>"),
        ("<KeyCount>1</KeyCount>", "<KeyCount>0</KeyCount>"),
        ("<Key>models/a.bin</Key>", "<Key>other/a.bin</Key>"),
        ("<Key>models/a.bin</Key>", "<Key>models/a/../b</Key>"),
        ("<Key>models/a.bin</Key>", "<Key>models/directory/</Key>"),
        ("<Size>8</Size>", "<Size>-1</Size>"),
        ("<Size>8</Size>", ""),
        ("<Key>models/a.bin</Key>", ""),
        ("<ETag>\"selected\"</ETag>", "<ETag>W/\"weak\"</ETag>"),
        ("<ETag>\"selected\"</ETag>", ""),
        (
            "</ListBucketResult>",
            "<ContinuationToken>unrequested</ContinuationToken></ListBucketResult>",
        ),
        (
            "</ListBucketResult>",
            "<Delimiter>/</Delimiter></ListBucketResult>",
        ),
        (
            "</ListBucketResult>",
            "<EncodingType>url</EncodingType></ListBucketResult>",
        ),
        (
            "</ListBucketResult>",
            "<StartAfter>models/0</StartAfter></ListBucketResult>",
        ),
        (
            "</ListBucketResult>",
            "<CommonPrefixes><Prefix>models/sub/</Prefix></CommonPrefixes></ListBucketResult>",
        ),
    ] {
        let fixture = Fixture::serve(vec![xml(&valid.replace(from, to))]).await;
        assert!(anonymous(fixture.endpoint.clone())
            .enumerate_prefix("models", limits())
            .await
            .is_err());
        assert_eq!(fixture.finish().await.len(), 1);
    }
}

#[tokio::test]
async fn duplicate_and_out_of_order_keys_or_cyclic_tokens_are_refused() {
    for responses in [
        vec![page(
            "models",
            &[("models/a", 8), ("models/a", 8)],
            false,
            None,
            None,
            2,
        )],
        vec![page(
            "models",
            &[("models/b", 8), ("models/a", 8)],
            false,
            None,
            None,
            2,
        )],
        vec![
            page("models", &[("models/a", 8)], true, None, Some("next"), 2),
            page("models", &[("models/a", 8)], false, Some("next"), None, 2),
        ],
        vec![
            page("models", &[], true, None, Some("one"), 2),
            page("models", &[], true, Some("one"), Some("one"), 2),
        ],
        vec![
            page("models", &[], true, None, Some("one"), 2),
            page("models", &[], true, Some("one"), Some("two"), 2),
            page("models", &[], true, Some("two"), Some("one"), 2),
        ],
        vec![
            page("models", &[], true, None, Some("one"), 2),
            page("models", &[], false, Some("wrong"), None, 2),
        ],
    ] {
        let count = responses.len();
        let fixture = Fixture::serve(responses.iter().map(|body| xml(body)).collect()).await;
        assert!(anonymous(fixture.endpoint.clone())
            .enumerate_prefix("models", limits())
            .await
            .is_err());
        assert_eq!(fixture.finish().await.len(), count);
    }
}

#[tokio::test]
async fn capacity_exhaustion_and_exact_xml_boundaries_do_not_shorten_results() {
    let truncated = page("models", &[("models/a", 8)], true, None, Some("next"), 1);
    for bound in [
        S3PrefixLimits {
            page_size: 1,
            max_pages: 1,
            ..limits()
        },
        S3PrefixLimits {
            page_size: 1,
            max_objects: 1,
            ..limits()
        },
        S3PrefixLimits {
            page_size: 1,
            max_total_bytes: truncated.len(),
            ..limits()
        },
    ] {
        let fixture = Fixture::serve(vec![xml(&truncated)]).await;
        assert!(matches!(
            anonymous(fixture.endpoint.clone())
                .enumerate_prefix("models", bound)
                .await,
            Err(S3PrefixError::Incomplete(_))
        ));
        assert_eq!(fixture.finish().await.len(), 1);
    }
    let complete = page("models", &[("models/a", 8)], false, None, None, 1);
    for accepted in [false, true] {
        let mut responses = vec![xml(&complete)];
        if accepted {
            responses.push(head(Some("v1"), Some("\"selected\"")));
        }
        let fixture = Fixture::serve(responses).await;
        let result = anonymous(fixture.endpoint.clone())
            .enumerate_prefix(
                "models",
                S3PrefixLimits {
                    page_size: 1,
                    max_objects: 1,
                    max_page_bytes: complete.len() - usize::from(!accepted),
                    max_total_bytes: complete.len(),
                    ..limits()
                },
            )
            .await;
        assert_eq!(result.is_ok(), accepted);
        assert_eq!(fixture.finish().await.len(), 1 + usize::from(accepted));
    }
    let too_many = page(
        "models",
        &[("models/a", 8), ("models/b", 8)],
        false,
        None,
        None,
        1,
    );
    let fixture = Fixture::serve(vec![xml(&too_many)]).await;
    assert!(anonymous(fixture.endpoint.clone())
        .enumerate_prefix(
            "models",
            S3PrefixLimits {
                page_size: 1,
                ..limits()
            }
        )
        .await
        .is_err());
    assert_eq!(fixture.finish().await.len(), 1);
}

#[tokio::test]
async fn empty_complete_and_empty_advancing_pages_are_honest() {
    for prefix in ["", "models/"] {
        let first = page(prefix, &[], true, None, Some("next"), 2);
        let final_page = page(prefix, &[], false, Some("next"), None, 2);
        let fixture = Fixture::serve(vec![xml(&first), xml(&final_page)]).await;
        let listing = anonymous(fixture.endpoint.clone())
            .enumerate_prefix(prefix, limits())
            .await
            .unwrap();
        assert!(listing.objects().is_empty());
        assert_eq!(listing.pages(), 2);
        assert_eq!(fixture.finish().await.len(), 2);
    }
}

#[tokio::test]
async fn list_to_head_races_or_mutable_missing_versions_fail_closed() {
    for response in [
        head(Some("null"), Some("\"selected\"")),
        head(None, Some("\"selected\"")),
        head(Some("v1"), Some("\"changed\"")),
        head(Some("v1"), Some("\"selected\"")).replace("Content-Length: 8", "Content-Length: 9"),
        "HTTP/1.1 412 Precondition Failed\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".into(),
        "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".into(),
    ] {
        let fixture = Fixture::serve(vec![
            xml(&page("models", &[("models/a", 8)], false, None, None, 2)),
            response,
        ])
        .await;
        let result = anonymous(fixture.endpoint.clone())
            .enumerate_prefix("models", limits())
            .await;
        assert!(matches!(
            result,
            Err(S3PrefixError::Reader(
                S3ReaderError::Changed | S3ReaderError::Unavailable
            ))
        ));
        let requests = fixture.finish().await;
        assert_eq!(requests.len(), 2);
        assert!(requests[1].contains("if-match: \"selected\""));
    }
}

#[tokio::test]
async fn namespaced_escaped_keys_and_zero_size_keep_exact_pins() {
    let body = page("models", &[("models/a&1", 0)], false, None, None, 2).replace(
        "<ListBucketResult>",
        "<ListBucketResult xmlns='http://s3.amazonaws.com/doc/2006-03-01/'>",
    );
    let fixture = Fixture::serve(vec![
        xml(&body),
        head(Some("v1"), Some("\"selected\"")).replace("Content-Length: 8", "Content-Length: 0"),
    ])
    .await;
    let result = anonymous(fixture.endpoint.clone())
        .enumerate_prefix("models", limits())
        .await
        .unwrap();
    assert_eq!(result.objects()[0].key(), "models/a&1");
    assert_eq!(result.objects()[0].size(), 0);
    assert_eq!(fixture.finish().await.len(), 2);
}

pub(super) async fn ambient_prefix_probe() {
    let responses = vec![
        xml(&page("models", &[("models/a", 8)], false, None, None, 2)),
        head(Some("v1"), Some("\"selected\"")),
    ];
    let fixture = Fixture::serve(std::iter::repeat_n(responses, 2).flatten().collect()).await;
    anonymous(fixture.endpoint.clone())
        .enumerate_prefix("models", limits())
        .await
        .unwrap();
    auth_tests::reader(fixture.endpoint.clone(), S3Addressing::Path, Some(TOKEN))
        .enumerate_prefix("models", limits())
        .await
        .unwrap();
    let requests = fixture.finish().await;
    assert_eq!(requests.len(), 4);
    assert!(requests[..2]
        .iter()
        .all(|r| !r.to_ascii_lowercase().contains("authorization:")));
    assert!(requests[2..]
        .iter()
        .all(|r| auth_tests::valid_signature(r, SECRET)));
    assert!(requests.iter().all(|r| !r.contains("ambient-synthetic")));
}

#[tokio::test]
async fn listing_errors_never_redirect_retry_or_expose_provider_diagnostics() {
    let receiver = Fixture::serve(vec![]).await;
    for status in [301, 302, 307, 308, 400, 401, 403, 404, 412, 429, 500, 503] {
        let body = format!(
            "<Error><Code>AccessDenied</Code><Message>{ACCESS} {SECRET} {TOKEN}</Message></Error>"
        );
        let response=format!("HTTP/1.1 {status} Fixture\r\nContent-Length: {}\r\nContent-Type: application/xml\r\nLocation: {}\r\nx-amz-bucket-region: another-region\r\nConnection: close\r\n\r\n{body}",body.len(),receiver.endpoint);
        let fixture = Fixture::serve(vec![response]).await;
        let error = auth_tests::reader(fixture.endpoint.clone(), S3Addressing::Path, Some(TOKEN))
            .enumerate_prefix("models", limits())
            .await
            .err()
            .unwrap();
        let public = format!("{error} {error:?}");
        for value in [ACCESS, SECRET, TOKEN] {
            assert!(!public.contains(value));
        }
        assert_eq!(fixture.finish().await.len(), 1);
    }
    assert!(receiver.finish().await.is_empty());
    for status in [201, 206] {
        let body = page("models", &[("models/a", 8)], false, None, None, 2);
        let fixture = Fixture::serve(vec![xml(&body).replacen(
            "200 OK",
            &format!("{status} Fixture"),
            1,
        )])
        .await;
        assert!(anonymous(fixture.endpoint.clone())
            .enumerate_prefix("models", limits())
            .await
            .is_err());
        assert_eq!(fixture.finish().await.len(), 1);
    }
}

#[test]
fn listing_and_head_polls_remain_redacted_under_global_trace() {
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "acquisition::s3::prefix_tests::global_trace_prefix_child",
            "--ignored",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "isolated prefix TRACE fixture failed"
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed"));
}

#[tokio::test]
#[ignore = "fresh process global subscriber fixture, executed by parent test"]
async fn global_trace_prefix_child() {
    let memory = auth_tests::MemoryLog(Arc::new(std::sync::Mutex::new(Vec::new())));
    let subscriber = tracing_subscriber::fmt()
        .without_time()
        .with_ansi(false)
        .with_max_level(tracing::Level::TRACE)
        .with_writer(memory.clone())
        .finish();
    tracing::subscriber::set_global_default(subscriber).unwrap();
    tracing::info!(phase = "discovery", "Pumas safe acquisition status");
    let fixture = Fixture::serve(vec![
        xml(&page(
            "models",
            &[("models/private-key", 8)],
            false,
            None,
            None,
            2,
        )),
        head(Some("private-version"), Some("\"selected\"")),
        xml(&page("models", &[], true, None, Some(TOKEN), 2).replace(
            "<IsTruncated>true</IsTruncated>",
            "<IsTruncated>true</IsTruncated><IsTruncated>false</IsTruncated>",
        )),
    ])
    .await;
    let reader = auth_tests::reader(fixture.endpoint.clone(), S3Addressing::Path, Some(TOKEN));
    let listing = reader.enumerate_prefix("models", limits()).await.unwrap();
    let error = reader
        .enumerate_prefix("models", limits())
        .await
        .err()
        .unwrap();
    tracing::info!(phase = "discovered", "Pumas safe acquisition status");
    let public = format!("{listing:?} {:?} {error} {error:?}", listing.objects());
    assert_eq!(fixture.finish().await.len(), 3);
    let buffer = memory.0.lock().unwrap();
    let logs = String::from_utf8_lossy(&buffer);
    assert!(logs.contains("discovery") && logs.contains("discovered"));
    for value in [
        ACCESS,
        SECRET,
        TOKEN,
        "models/private-key",
        "private-version",
    ] {
        assert!(
            !logs.contains(value),
            "private source material leaked in prefix tracing"
        );
        assert!(!public.contains(value));
    }
}

#[tokio::test]
async fn dropping_or_timing_out_stalled_listing_drains_the_connection() {
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };
    for cancel in [false, true] {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let (started, mut waiting) = tokio::sync::oneshot::channel();
        let source = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let request = fixture::request(&mut socket).await;
            assert!(request.contains("list-type=2"));
            socket
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 4096\r\nConnection: close\r\n\r\n<")
                .await
                .unwrap();
            started.send(()).unwrap();
            let mut byte = [0];
            assert_eq!(
                socket.read(&mut byte).await.unwrap(),
                0,
                "listing custody must drain on cancellation/deadline"
            );
        });
        let mut config = auth_tests::config(endpoint, S3Addressing::Path);
        config.operation_timeout = if cancel {
            Duration::from_secs(5)
        } else {
            Duration::from_millis(100)
        };
        let reader = S3Reader::new(config).unwrap();
        let mut operation = Box::pin(reader.enumerate_prefix("models", limits()));
        tokio::select! { _=&mut waiting=>{},result=&mut operation=>panic!("listing returned before the controlled stall: {result:?}") }
        if cancel {
            drop(operation);
        } else {
            // The existing reqwest budget can expire before the outer Tokio
            // budget. Its contained connector error is deliberately static;
            // do not invent a new provider error or retry classification.
            let result = operation.await;
            println!("bounded unfinished listing result: {result:?}");
            assert!(matches!(
                result,
                Err(S3PrefixError::Reader(
                    S3ReaderError::TimedOut | S3ReaderError::Protocol(_)
                ))
            ));
        }
        tokio::time::timeout(Duration::from_secs(5), source)
            .await
            .unwrap()
            .unwrap();
    }
}

#[tokio::test]
async fn wire_byte_overflow_is_refused_before_body_eof() {
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let source = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let _ = fixture::request(&mut socket).await;
        socket
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 4096\r\nConnection: close\r\n\r\n")
            .await
            .unwrap();
        socket.write_all(&[b'x'; 256]).await.unwrap();
        let mut byte = [0];
        assert_eq!(
            socket.read(&mut byte).await.unwrap(),
            0,
            "byte overflow must close the unfinished listing"
        );
    });
    let result = tokio::time::timeout(
        Duration::from_secs(2),
        anonymous(endpoint).enumerate_prefix(
            "models",
            S3PrefixLimits {
                max_page_bytes: 32,
                ..limits()
            },
        ),
    )
    .await
    .unwrap();
    assert!(matches!(
        result,
        Err(S3PrefixError::Reader(S3ReaderError::Protocol(_)))
    ));
    tokio::time::timeout(Duration::from_secs(5), source)
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn one_caller_budget_covers_listing_and_subsequent_head_pin() {
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let source = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let _ = fixture::request(&mut socket).await;
        tokio::time::sleep(Duration::from_millis(100)).await;
        socket
            .write_all(xml(&page("models", &[("models/a", 8)], false, None, None, 2)).as_bytes())
            .await
            .unwrap();
        socket.shutdown().await.unwrap();
        let (mut socket, _) = listener.accept().await.unwrap();
        assert!(fixture::request(&mut socket).await.starts_with("HEAD "));
        // Relative to this HEAD, 150ms is within the per-request 200ms budget,
        // but it exceeds the remaining total enumeration budget.
        let start = std::time::Instant::now();
        let mut byte = [0];
        assert_eq!(socket.read(&mut byte).await.unwrap(), 0);
        assert!(start.elapsed() < Duration::from_millis(180));
    });
    let mut config = auth_tests::config(endpoint, S3Addressing::Path);
    config.operation_timeout = Duration::from_millis(200);
    assert!(matches!(
        S3Reader::new(config)
            .unwrap()
            .enumerate_prefix("models", limits())
            .await,
        Err(S3PrefixError::Reader(S3ReaderError::TimedOut))
    ));
    tokio::time::timeout(Duration::from_secs(5), source)
        .await
        .unwrap()
        .unwrap();
}
